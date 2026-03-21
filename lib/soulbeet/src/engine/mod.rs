pub mod blender;
pub mod diversifier;
pub mod freshness;
pub mod lastfm_pipeline;
pub mod listenbrainz_pipeline;
pub mod profile;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tracing::{info, warn};

use crate::error::Result;
use crate::traits::{CandidateGenerator, ScrobbleProvider};
use shared::recommendation::{
    Candidate, CandidateSet, EngineReport, PipelineReport, ProfileConfig, ProfileSummary,
    TimePeriod, UserMusicProfile,
};

const ARTIST_CACHE_LIMIT: usize = 100;

/// Per-pipeline-run cache for artist popularity and genre metadata.
/// Avoids redundant API calls when the same artist appears across signals.
/// Stops fetching new metadata after encountering `ARTIST_CACHE_LIMIT` unique artists.
pub(crate) struct ArtistCache {
    pub popularity: HashMap<String, u64>,
    pub genre: HashMap<String, String>,
    fetch_count: usize,
}

impl ArtistCache {
    pub fn new() -> Self {
        Self {
            popularity: HashMap::new(),
            genre: HashMap::new(),
            fetch_count: 0,
        }
    }

    pub async fn get_popularity(
        &mut self,
        provider: &dyn ScrobbleProvider,
        artist: &str,
    ) -> Option<u64> {
        let key = artist.to_lowercase();
        if let Some(&count) = self.popularity.get(&key) {
            return Some(count);
        }
        if self.fetch_count >= ARTIST_CACHE_LIMIT {
            return None;
        }
        if let Ok(pop) = provider.get_artist_popularity(artist).await {
            self.fetch_count += 1;
            self.popularity.insert(key, pop.listener_count);
            return Some(pop.listener_count);
        }
        None
    }

    pub async fn get_genre(
        &mut self,
        provider: &dyn ScrobbleProvider,
        artist: &str,
    ) -> Option<String> {
        let key = artist.to_lowercase();
        if let Some(genre) = self.genre.get(&key) {
            return Some(genre.clone());
        }
        if self.fetch_count >= ARTIST_CACHE_LIMIT {
            return None;
        }
        if let Ok(tags) = provider.get_artist_tags(artist).await {
            self.fetch_count += 1;
            if let Some(top) = tags.first() {
                self.genre.insert(key, top.name.clone());
                return Some(top.name.clone());
            }
        }
        None
    }
}

pub use lastfm_pipeline::LastFmPipeline;
pub use listenbrainz_pipeline::ListenBrainzPipeline;
pub use profile::build_profile;

/// Run the full recommendation pipeline with a pre-built profile.
///
/// 1. Generate candidates from each generator.
/// 2. Blend results across sources.
/// 3. Apply freshness penalties.
/// 4. Diversify and select the final list.
///
/// Returns the final candidates alongside a structured report of all decisions.
pub async fn recommend(
    providers: &[Arc<dyn ScrobbleProvider>],
    generators: &[Arc<dyn CandidateGenerator>],
    profile: &UserMusicProfile,
    config: &ProfileConfig,
    target_count: usize,
) -> Result<(Vec<Candidate>, EngineReport)> {
    let start = std::time::Instant::now();

    info!(
        "Starting recommendation: {} generators, target {}",
        generators.len(),
        target_count
    );

    let mut report = EngineReport::default();

    // Populate profile summary
    report.profile_summary = ProfileSummary {
        genre_count: profile.genre_distribution.len(),
        top_genres: profile
            .genre_distribution
            .iter()
            .take(5)
            .map(|t| t.name.clone())
            .collect(),
        obscurity_score: profile.obscurity_score,
        repeat_ratio: profile.repeat_ratio,
        freshness_half_life_days: profile.freshness_half_life_days,
        momentum_artists: profile
            .momentum_artists
            .iter()
            .map(|a| a.name.clone())
            .collect(),
        comfort_tags: profile.tag_comfort_zone.len(),
        exploration_tags: profile.tag_exploration_zone.len(),
    };

    // Step 1: Generate candidates from each pipeline.
    let mut source_sets: Vec<(&str, CandidateSet)> = Vec::new();

    for generator in generators {
        // Per-pipeline timeout: 2 minutes max
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3600),
            generator.generate_candidates(profile, config),
        )
        .await;

        let result = match result {
            Ok(r) => r,
            Err(_) => {
                warn!("Generator '{}' timed out after 3600s", generator.name());
                report.pipeline_reports.push(PipelineReport {
                    name: format!("{} (TIMEOUT)", generator.name()),
                    signals: vec![],
                    total_candidates: 0,
                });
                continue;
            }
        };

        match result {
            Ok((set, signal_reports)) => {
                info!(
                    "Generator '{}' produced {} candidates",
                    generator.name(),
                    set.len()
                );
                report.pipeline_reports.push(PipelineReport {
                    name: generator.name().to_string(),
                    signals: signal_reports,
                    total_candidates: set.len(),
                });
                source_sets.push((generator.name(), set));
            }
            Err(e) => {
                warn!("Generator '{}' failed: {}", generator.name(), e);
                report.pipeline_reports.push(PipelineReport {
                    name: format!("{} (ERROR: {})", generator.name(), e),
                    signals: vec![],
                    total_candidates: 0,
                });
            }
        }
    }

    if source_sets.is_empty() {
        warn!("No generators produced candidates");
        report.duration_secs = start.elapsed().as_secs_f64();
        return Ok((vec![], report));
    }

    // Step 2: Blend
    let (mut blended, blend_summary) = blender::blend(source_sets, config);
    info!("After blending: {} candidates", blended.len());
    report.blend_summary = blend_summary;

    // Step 3: Freshness
    let known_artists = collect_known_artists(providers).await;
    let freshness_summary =
        freshness::apply_freshness(&mut blended, profile, &known_artists, config);
    report.freshness_summary = freshness_summary;

    // Step 4: Diversify
    let (result, diversifier_summary) =
        diversifier::diversify(blended, profile, config, target_count);
    info!("Final recommendation: {} tracks", result.len());
    report.diversifier_summary = diversifier_summary;

    report.final_count = result.len();
    report.duration_secs = start.elapsed().as_secs_f64();

    Ok((result, report))
}

/// Build a user profile from the best available provider, then run the pipeline.
///
/// Tries each provider in order and uses the first one that returns meaningful
/// data (at least 1 top artist). This handles the case where a user has data
/// on Last.fm but not ListenBrainz (or vice versa).
pub async fn build_and_recommend(
    providers: &[Arc<dyn ScrobbleProvider>],
    generators: &[Arc<dyn CandidateGenerator>],
    _profile_provider: &dyn ScrobbleProvider,
    config: &ProfileConfig,
    target_count: usize,
) -> Result<(UserMusicProfile, Vec<Candidate>, EngineReport)> {
    let mut profile = UserMusicProfile::default();
    let mut profile_source = String::from("none");
    for provider in providers {
        info!("Trying profile build from {}", provider.name());
        match build_profile(provider.as_ref()).await {
            Ok(p) if !p.genre_distribution.is_empty() || !p.momentum_artists.is_empty() => {
                info!(
                    "Profile built from {} with {} genres, {} momentum artists",
                    provider.name(),
                    p.genre_distribution.len(),
                    p.momentum_artists.len()
                );
                profile_source = provider.name().to_string();
                profile = p;
                break;
            }
            Ok(_) => {
                info!(
                    "{} returned empty profile, trying next provider",
                    provider.name()
                );
            }
            Err(e) => {
                warn!(
                    "Profile build from {} failed: {}, trying next",
                    provider.name(),
                    e
                );
            }
        }
    }
    let (recommendations, mut report) =
        recommend(providers, generators, &profile, config, target_count).await?;
    report.profile_source = profile_source;
    Ok((profile, recommendations, report))
}

/// Collect a set of known artist names (lowercased) from all providers.
async fn collect_known_artists(providers: &[Arc<dyn ScrobbleProvider>]) -> HashSet<String> {
    let mut known = HashSet::new();
    for provider in providers {
        match provider.get_top_artists(TimePeriod::AllTime, 200).await {
            Ok(artists) => {
                for a in artists {
                    known.insert(a.name.to_lowercase());
                }
            }
            Err(e) => {
                warn!(
                    "Failed to fetch known artists from {}: {}",
                    provider.name(),
                    e
                );
            }
        }
    }
    known
}
