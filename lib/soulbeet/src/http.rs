use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use reqwest::{Client, RequestBuilder, Response};
use tokio::sync::Mutex;
use tracing::warn;

use crate::error::{Result, SoulseekError};

const MAX_RETRIES: u32 = 3;
const BASE_DELAY_MS: u64 = 500;
const MAX_DELAY_MS: u64 = 5000;

/// Status codes that warrant a retry (server-side transient errors).
fn is_retryable(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

/// Execute an HTTP request with retries and exponential backoff.
///
/// Retries on network errors, 429, 500, 502, 503, 504.
/// Does NOT retry on 4xx client errors (except 429).
pub async fn resilient_send(
    build_request: impl Fn() -> RequestBuilder,
    context: &str,
) -> Result<Response> {
    let mut last_err = SoulseekError::Api {
        status: 0,
        message: format!("{}: no attempts made", context),
    };

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = (BASE_DELAY_MS * 2u64.pow(attempt - 1)).min(MAX_DELAY_MS);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        let resp = match build_request().send().await {
            Ok(r) => r,
            Err(e) => {
                warn!("{}: attempt {} network error: {}", context, attempt + 1, e);
                last_err = SoulseekError::Api {
                    status: 0,
                    message: format!("{}: {}", context, e),
                };
                continue;
            }
        };

        let status = resp.status().as_u16();

        if resp.status().is_success() || status == 204 {
            return Ok(resp);
        }

        if is_retryable(status) && attempt < MAX_RETRIES {
            warn!(
                "{}: attempt {} got {}, retrying",
                context,
                attempt + 1,
                status
            );
            last_err = SoulseekError::Api {
                status,
                message: format!("{}: HTTP {}", context, status),
            };
            continue;
        }

        let body = resp.text().await.unwrap_or_default();
        return Err(SoulseekError::Api {
            status,
            message: format!("{} ({}): {}", context, status, body),
        });
    }

    Err(last_err)
}

/// Build a reqwest Client with standard timeouts.
pub fn build_client(user_agent: &str) -> Client {
    Client::builder()
        .user_agent(user_agent)
        .timeout(Duration::from_secs(15))
        .connect_timeout(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .expect("failed to build HTTP client")
}

// --- Per-service rate limiters ---
//
// Each service has its own limiter so they don't block each other.
// The interval is the minimum time between requests to that service.

struct RateLimiter(Mutex<Instant>);

impl RateLimiter {
    fn new() -> Self {
        Self(Mutex::new(Instant::now() - Duration::from_secs(2)))
    }

    async fn wait(&self, min_interval: Duration) {
        let mut last = self.0.lock().await;
        let elapsed = last.elapsed();
        if elapsed < min_interval {
            tokio::time::sleep(min_interval - elapsed).await;
        }
        *last = Instant::now();
    }
}

// MusicBrainz: 1 req/sec (documented hard limit, 503 if exceeded)
static MB_LIMITER: LazyLock<RateLimiter> = LazyLock::new(RateLimiter::new);
// Last.fm: 1 req/sec (undocumented, matching MB for safety)
static LFM_LIMITER: LazyLock<RateLimiter> = LazyLock::new(RateLimiter::new);
// ListenBrainz: ~2 req/sec (uses response headers, but we preemptively limit)
static LB_LIMITER: LazyLock<RateLimiter> = LazyLock::new(RateLimiter::new);

const MB_INTERVAL: Duration = Duration::from_millis(1100);
const LFM_INTERVAL: Duration = Duration::from_millis(1000);
const LB_INTERVAL: Duration = Duration::from_millis(500);

pub async fn mb_rate_limit() {
    MB_LIMITER.wait(MB_INTERVAL).await;
}

pub async fn lastfm_rate_limit() {
    LFM_LIMITER.wait(LFM_INTERVAL).await;
}

pub async fn lb_rate_limit() {
    LB_LIMITER.wait(LB_INTERVAL).await;
}

// --- MBID cache (avoids repeated MusicBrainz lookups for the same artist) ---

static MBID_CACHE: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Look up an artist MBID, checking the cache first.
/// Returns None if not found or if MusicBrainz doesn't have a match.
pub async fn cached_mbid_lookup(client: &Client, artist: &str) -> Result<Option<String>> {
    let key = artist.to_lowercase();

    // Check cache
    {
        let cache = MBID_CACHE.lock().await;
        if let Some(cached) = cache.get(&key) {
            return Ok(cached.clone());
        }
    }

    // Rate limit, then fetch
    mb_rate_limit().await;

    let url = format!(
        "https://musicbrainz.org/ws/2/artist/?query=artist:{}&fmt=json&limit=1",
        url::form_urlencoded::byte_serialize(artist.as_bytes()).collect::<String>()
    );

    let client_clone = client.clone();
    let url_clone = url.clone();
    let resp = match resilient_send(
        || client_clone.get(&url_clone),
        &format!("MB lookup {}", artist),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("MusicBrainz lookup failed for '{}': {}", artist, e);
            // Cache the failure so we don't retry immediately
            MBID_CACHE.lock().await.insert(key, None);
            return Ok(None);
        }
    };

    if !resp.status().is_success() {
        MBID_CACHE.lock().await.insert(key, None);
        return Ok(None);
    }

    #[derive(serde::Deserialize)]
    struct MbResponse {
        #[serde(default)]
        artists: Vec<MbArtist>,
    }
    #[derive(serde::Deserialize)]
    struct MbArtist {
        id: String,
        #[serde(default)]
        score: Option<u32>,
    }

    let data: MbResponse = resp.json().await.map_err(|e| SoulseekError::Api {
        status: 500,
        message: format!("Failed to parse MusicBrainz response: {}", e),
    })?;

    let result = data.artists.into_iter().next().and_then(|a| {
        if a.score.unwrap_or(0) >= 90 {
            Some(a.id)
        } else {
            None
        }
    });

    // Cache the result
    MBID_CACHE.lock().await.insert(key, result.clone());
    Ok(result)
}
