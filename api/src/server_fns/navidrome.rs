use dioxus::prelude::*;
use shared::navidrome::{DeletionReview, LibraryStats, SyncResult};

#[cfg(feature = "server")]
use dioxus::logger::tracing::{info, warn};

#[cfg(feature = "server")]
use crate::models::deletion_review::DeletionReviewRow;
#[cfg(feature = "server")]
use crate::models::discovery_history::DiscoveryHistoryRow;
#[cfg(feature = "server")]
use crate::models::discovery_playlist::DiscoveryTrackRow;
#[cfg(feature = "server")]
use crate::models::user_settings::UserSettings;
#[cfg(feature = "server")]
use crate::services::navidrome_client_for_user;
#[cfg(feature = "server")]
use crate::AuthSession;
#[cfg(feature = "server")]
use shared::navidrome::DiscoveryStatus;

#[cfg(feature = "server")]
use super::server_error;

#[post("/api/navidrome/sync-ratings", auth: AuthSession)]
pub async fn sync_ratings() -> Result<SyncResult, ServerFnError> {
    sync_ratings_internal(&auth.0.sub)
        .await
        .map_err(server_error)
}

#[cfg(feature = "server")]
pub async fn sync_ratings_internal(user_id: &str) -> Result<SyncResult, String> {
    let client = navidrome_client_for_user(user_id).await?;
    let songs = client
        .get_all_songs_with_ratings()
        .await
        .map_err(|e| e.to_string())?;

    let total_songs_scanned = songs.len() as u32;

    let user_settings = UserSettings::get(user_id).await?;
    let promote_threshold = user_settings.discovery_promote_threshold;
    let auto_delete = user_settings.auto_delete_enabled;

    // Build music root candidates for resolving Navidrome's relative paths.
    // Try multiple strategies: env var, folder paths, folder parents.
    let folders = crate::models::folder::Folder::get_all_by_user(user_id)
        .await
        .unwrap_or_default();
    let music_roots: Vec<std::path::PathBuf> = {
        let mut roots = Vec::new();
        // 1. NAVIDROME_MUSIC_PATH env var (most reliable if set)
        if let Ok(p) = std::env::var("NAVIDROME_MUSIC_PATH") {
            if !p.is_empty() {
                roots.push(std::path::PathBuf::from(p));
            }
        }
        // 2. Each folder path directly (user folder might BE the music root)
        for f in &folders {
            roots.push(std::path::PathBuf::from(&f.path));
        }
        // 3. Parents of folder paths (e.g. /music from /music/Person1)
        for f in &folders {
            if let Some(parent) = std::path::Path::new(&f.path).parent() {
                roots.push(parent.to_path_buf());
            }
        }
        roots.dedup();
        roots
    };
    if music_roots.is_empty() {
        warn!("No music roots found for user {} (no folders configured, no NAVIDROME_MUSIC_PATH)", user_id);
    }

    let mut deleted_tracks = 0u32;
    let mut promoted_tracks = 0u32;
    let mut removed_tracks = 0u32;
    let mut skipped_veto = 0u32;
    let mut skipped_not_found = 0u32;

    let pending_discovery_tracks = DiscoveryTrackRow::get_all_pending().await?;

    for song in &songs {
        // Auto-delete 1-star tracks (when enabled)
        if auto_delete {
            if let Some(rating) = song.user_rating {
                if rating == 1 {
                    let shared_veto = song
                        .average_rating
                        .map(|avg| avg > 1.0)
                        .unwrap_or(false);
                    if shared_veto {
                        info!(
                            "Auto-delete skipped (shared veto, avg={:.1}): {} - {}",
                            song.average_rating.unwrap_or(0.0),
                            song.artist.as_deref().unwrap_or("?"),
                            song.title
                        );
                        skipped_veto += 1;
                    } else if let Some(ref path_str) = song.path {
                        let resolved = resolve_song_path(path_str, &music_roots);
                        match resolved {
                            Some(path) => {
                                if let Err(e) = tokio::fs::remove_file(&path).await {
                                    warn!("Auto-delete failed for {}: {}", path.display(), e);
                                } else {
                                    if let Some(parent) = path.parent() {
                                        let _ = cleanup_empty_dirs(parent).await;
                                    }
                                    DeletionReviewRow::upsert(
                                        &song.id,
                                        &song.title,
                                        song.artist.as_deref().unwrap_or("Unknown"),
                                        song.album.as_deref().unwrap_or("Unknown"),
                                        Some(&path.to_string_lossy()),
                                        Some(rating),
                                        user_id,
                                    )
                                    .await?;
                                    deleted_tracks += 1;
                                }
                            }
                            None => {
                                warn!(
                                    "Auto-delete skipped (file not found): {} - {} (path: {})",
                                    song.artist.as_deref().unwrap_or("?"),
                                    song.title,
                                    path_str
                                );
                                skipped_not_found += 1;
                            }
                        }
                    } else {
                        warn!(
                            "Auto-delete skipped (no path from Navidrome): {} - {}",
                            song.artist.as_deref().unwrap_or("?"),
                            song.title
                        );
                    }
                }
            }
        }

        // Check discovery track promotion/removal
        if let Some(user_rating) = song.user_rating {
            // Match by song_id first (exact), then by filename (fuzzy).
            // song_id is authoritative when set by reconciliation.
            let matching_track = pending_discovery_tracks.iter().find(|dt| {
                if let Some(ref dt_song_id) = dt.song_id {
                    return dt_song_id == &song.id;
                }
                // Fallback: match by filename when song_id isn't set yet
                if let Some(ref song_path) = song.path {
                    let song_fn = std::path::Path::new(song_path)
                        .file_name()
                        .map(|f| f.to_ascii_lowercase());
                    let dt_fn = std::path::Path::new(&dt.path)
                        .file_name()
                        .map(|f| f.to_ascii_lowercase());
                    return song_fn.is_some() && song_fn == dt_fn;
                }
                false
            });

            if let Some(track) = matching_track {
                if user_rating >= promote_threshold {
                    if let Err(e) = promote_discovery_track_internal(&track.id).await {
                        warn!("Failed to promote track {}: {}", track.title, e);
                    } else {
                        info!("Promoted discovery track: {} - {} (rating {})", track.artist, track.title, user_rating);
                        if let Err(e) = DiscoveryHistoryRow::update_outcome(
                            user_id,
                            &track.artist,
                            &track.title,
                            "promoted",
                        )
                        .await {
                            warn!("Failed to update history for promoted track '{}': {}", track.title, e);
                        }
                        promoted_tracks += 1;
                    }
                } else if user_rating == 1 {
                    if let Err(e) = remove_discovery_track_internal(&track.id).await {
                        warn!("Failed to remove track {}: {}", track.title, e);
                    } else {
                        info!("Removed discovery track: {} - {} (rating 1)", track.artist, track.title);
                        if let Err(e) = DiscoveryHistoryRow::update_outcome(
                            user_id,
                            &track.artist,
                            &track.title,
                            "removed",
                        )
                        .await {
                            warn!("Failed to update history for removed track '{}': {}", track.title, e);
                        }
                        removed_tracks += 1;
                    }
                }
            }
        }
    }

    if !auto_delete {
        info!("Auto-delete is disabled for this user");
    } else if skipped_veto > 0 || skipped_not_found > 0 {
        info!(
            "Auto-delete: {} skipped (shared veto), {} skipped (file not found)",
            skipped_veto, skipped_not_found
        );
    }
    info!(
        "Ratings sync complete: {} songs scanned, {} deleted, {} promoted, {} removed",
        total_songs_scanned, deleted_tracks, promoted_tracks, removed_tracks
    );

    Ok(SyncResult {
        deleted_tracks,
        promoted_tracks,
        removed_tracks,
        total_songs_scanned,
    })
}

#[get("/api/navidrome/deletion-history", auth: AuthSession)]
pub async fn get_deletion_history() -> Result<Vec<DeletionReview>, ServerFnError> {
    DeletionReviewRow::get_history(&auth.0.sub, 50)
        .await
        .map_err(server_error)
}

/// Resolve a song path from Navidrome to an absolute path on disk.
///
/// Navidrome returns paths relative to its music library root, which may not
/// exactly match the filesystem (different naming conventions, character
/// substitutions, stale paths). We try exact resolution first, then fall back
/// to searching by artist directory + fuzzy filename matching.
#[cfg(feature = "server")]
fn resolve_song_path(
    path_str: &str,
    music_roots: &[std::path::PathBuf],
) -> Option<std::path::PathBuf> {
    let path = std::path::Path::new(path_str);

    // Try as-is (handles absolute paths)
    if path.is_absolute() && path.exists() {
        return Some(path.to_path_buf());
    }

    // Try exact resolution against each music root
    for root in music_roots {
        let resolved = root.join(path_str);
        if resolved.exists() {
            return Some(resolved);
        }
    }

    // Fuzzy fallback: find by artist directory + filename stem.
    // Navidrome paths look like "Artist/Album/01-12 - Title.flac".
    // The actual file might be named differently (e.g. "12 Title.flac")
    // or in a slightly different folder (e.g. "Album!" vs "Album?").
    let components: Vec<_> = path.components().collect();
    if components.is_empty() {
        return None;
    }

    // Extract the artist directory (first component) and search within it
    let artist_dir = components[0].as_os_str().to_string_lossy();
    let file_stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    // Extract a clean title from the filename by stripping track numbers.
    // "01-12 - Do You Feel" -> "do you feel"
    // "12 Do You Feel" -> "do you feel"
    let clean_title = strip_track_prefix(&file_stem);

    for root in music_roots {
        let artist_path = root.join(artist_dir.as_ref());
        if !artist_path.is_dir() {
            continue;
        }
        // Search recursively within the artist directory
        if let Some(found) = find_file_by_title(&artist_path, &clean_title, &extension) {
            return Some(found);
        }
    }

    None
}

/// Strip track number prefixes from a filename stem.
/// "01-12 - do you feel" -> "do you feel"
/// "12 do you feel" -> "do you feel"
/// "01 - do you feel" -> "do you feel"
#[cfg(feature = "server")]
fn strip_track_prefix(stem: &str) -> String {
    // Try "01-12 - Title" or "01 - Title" pattern
    if let Some(idx) = stem.find(" - ") {
        return stem[idx + 3..].trim().to_string();
    }
    // Try "12 Title" pattern (digits followed by space)
    let trimmed = stem.trim_start_matches(|c: char| c.is_ascii_digit());
    if trimmed.len() < stem.len() {
        return trimmed.trim_start().to_string();
    }
    stem.to_string()
}

/// Recursively search for a file matching a title within a directory.
#[cfg(feature = "server")]
fn find_file_by_title(
    dir: &std::path::Path,
    clean_title: &str,
    extension: &str,
) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_by_title(&path, clean_title, extension) {
                return Some(found);
            }
        } else if path.is_file() {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext != extension {
                continue;
            }
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let candidate_title = strip_track_prefix(&stem);
            if candidate_title == clean_title {
                return Some(path);
            }
        }
    }
    None
}

/// Remove a directory if empty, then recurse up to its parent.
#[cfg(feature = "server")]
async fn cleanup_empty_dirs(dir: &std::path::Path) -> Result<(), std::io::Error> {
    let mut read_dir = tokio::fs::read_dir(dir).await?;
    if read_dir.next_entry().await?.is_none() {
        tokio::fs::remove_dir(dir).await?;
        if let Some(parent) = dir.parent() {
            Box::pin(cleanup_empty_dirs(parent)).await?;
        }
    }
    Ok(())
}

#[get("/api/navidrome/library-stats", auth: AuthSession)]
pub async fn get_library_stats() -> Result<LibraryStats, ServerFnError> {
    let client = navidrome_client_for_user(&auth.0.sub)
        .await
        .map_err(server_error)?;
    let songs = client
        .get_all_songs_with_ratings()
        .await
        .map_err(server_error)?;

    let albums = client.get_all_albums().await.map_err(server_error)?;

    let total_tracks = songs.len() as u32;
    let mut rated_tracks = 0u32;
    let mut rating_sum = 0.0f64;
    let mut rating_distribution = [0u32; 5];
    let mut genres: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut artists: std::collections::HashSet<String> = std::collections::HashSet::new();

    for song in &songs {
        if let Some(artist) = &song.artist {
            artists.insert(artist.to_lowercase());
        }
        if let Some(genre) = &song.genre {
            if !genre.is_empty() {
                *genres.entry(genre.clone()).or_default() += 1;
            }
        }
        if let Some(rating) = song.user_rating {
            if (1..=5).contains(&rating) {
                rated_tracks += 1;
                rating_sum += rating as f64;
                rating_distribution[(rating - 1) as usize] += 1;
            }
        }
    }

    let average_rating = if rated_tracks > 0 {
        rating_sum / rated_tracks as f64
    } else {
        0.0
    };

    let mut genre_vec: Vec<(String, u32)> = genres.into_iter().collect();
    genre_vec.sort_by(|a, b| b.1.cmp(&a.1));
    genre_vec.truncate(20);

    Ok(LibraryStats {
        total_tracks,
        rated_tracks,
        unrated_tracks: total_tracks - rated_tracks,
        average_rating,
        rating_distribution,
        total_albums: albums.len() as u32,
        total_artists: artists.len() as u32,
        genres: genre_vec,
    })
}

#[cfg(feature = "server")]
async fn promote_discovery_track_internal(track_id: &str) -> Result<(), String> {
    use crate::models::folder::Folder;

    let track = DiscoveryTrackRow::get_by_id(track_id)
        .await?
        .ok_or("Discovery track not found")?;

    let folder = Folder::get_by_id(&track.folder_id)
        .await?
        .ok_or("Folder not found")?;

    let src = std::path::PathBuf::from(&track.path);
    if !src.exists() {
        return Err(format!("Source file not found: {}", track.path));
    }

    // Import into parent library folder via beets for proper tagging
    let target = std::path::PathBuf::from(&folder.path);
    match crate::services::music_importer(None).await {
        Ok(imp) => {
            match imp.import(&[src.as_path()], &target, false).await {
                Ok(soulbeet::ImportResult::Success) => {}
                Ok(soulbeet::ImportResult::Skipped) => {
                    return Err("Beets skipped track (duplicate?)".to_string());
                }
                Ok(other) => {
                    return Err(format!("Import issue: {:?}", other));
                }
                Err(e) => {
                    return Err(format!("Import failed: {}", e));
                }
            }
        }
        Err(_) => {
            // Fallback: raw move
            let filename = src.file_name().ok_or("Invalid filename")?.to_string_lossy().to_string();
            let dest = target.join(&filename);
            if let Err(e) = tokio::fs::rename(&src, &dest).await {
                if e.raw_os_error() == Some(18) {
                    tokio::fs::copy(&src, &dest).await.map_err(|e| format!("Failed to copy: {}", e))?;
                    let _ = tokio::fs::remove_file(&src).await;
                } else {
                    return Err(format!("Failed to move: {}", e));
                }
            }
        }
    }

    DiscoveryTrackRow::update_status(track_id, &DiscoveryStatus::Promoted).await?;
    info!("Promoted discovery track: {} -> {}", track.title, folder.path);
    Ok(())
}

#[cfg(feature = "server")]
async fn remove_discovery_track_internal(track_id: &str) -> Result<(), String> {
    let track = DiscoveryTrackRow::get_by_id(track_id)
        .await?
        .ok_or("Discovery track not found")?;

    let path = std::path::Path::new(&track.path);
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .map_err(|e| format!("Failed to delete file: {}", e))?;
        if let Some(parent) = path.parent() {
            let _ = cleanup_empty_dirs(parent).await;
        }
    }

    DiscoveryTrackRow::update_status(track_id, &DiscoveryStatus::Removed).await?;

    info!("Removed discovery track: {}", track.title);
    Ok(())
}
