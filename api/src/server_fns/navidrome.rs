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

    let mut deleted_tracks = 0u32;
    let mut promoted_tracks = 0u32;
    let mut removed_tracks = 0u32;

    let pending_discovery_tracks = DiscoveryTrackRow::get_all_pending().await?;

    for song in &songs {
        // Auto-delete 1-star tracks (when enabled)
        if auto_delete {
            if let Some(rating) = song.user_rating {
                if rating == 1 {
                    // For shared folders: skip deletion if the average rating
                    // across all Navidrome users is above 1 (someone else likes it)
                    let shared_veto = song
                        .average_rating
                        .map(|avg| avg > 1.0)
                        .unwrap_or(false);
                    if !shared_veto {
                        if let Some(ref path_str) = song.path {
                            let path = std::path::Path::new(path_str);
                            if path.exists() {
                                if let Err(e) = tokio::fs::remove_file(path).await {
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
                                        song.path.as_deref(),
                                        Some(rating),
                                        user_id,
                                    )
                                    .await?;
                                    deleted_tracks += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check discovery track promotion/removal
        if let Some(user_rating) = song.user_rating {
            // Find matching discovery track by song_id or path
            let matching_track = pending_discovery_tracks.iter().find(|dt| {
                dt.song_id.as_deref() == Some(&song.id)
                    || song
                        .path
                        .as_ref()
                        .map(|p| p.ends_with(&dt.path) || dt.path.ends_with(p))
                        .unwrap_or(false)
            });

            if let Some(track) = matching_track {
                if user_rating >= promote_threshold {
                    // Promote: move file from Discovery/ to parent folder
                    if let Err(e) = promote_discovery_track_internal(&track.id).await {
                        warn!("Failed to promote track {}: {}", track.title, e);
                    } else {
                        DiscoveryHistoryRow::update_outcome(
                            user_id,
                            &track.artist,
                            &track.title,
                            "promoted",
                        )
                        .await
                        .ok();
                        promoted_tracks += 1;
                    }
                } else if user_rating == 1 {
                    // Remove: delete the file
                    if let Err(e) = remove_discovery_track_internal(&track.id).await {
                        warn!("Failed to remove track {}: {}", track.title, e);
                    } else {
                        DiscoveryHistoryRow::update_outcome(
                            user_id,
                            &track.artist,
                            &track.title,
                            "removed",
                        )
                        .await
                        .ok();
                        removed_tracks += 1;
                    }
                }
            }
        }
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

#[cfg(feature = "server")]
async fn cleanup_empty_dirs(dir: &std::path::Path) -> Result<(), std::io::Error> {
    let mut read_dir = tokio::fs::read_dir(dir).await?;
    if read_dir.next_entry().await?.is_none() {
        tokio::fs::remove_dir(dir).await?;
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

    let filename = src
        .file_name()
        .ok_or("Invalid filename")?
        .to_string_lossy()
        .to_string();
    let dest = std::path::PathBuf::from(&folder.path).join(&filename);

    if let Err(e) = tokio::fs::rename(&src, &dest).await {
        if e.raw_os_error() == Some(18) {
            tokio::fs::copy(&src, &dest)
                .await
                .map_err(|e| format!("Failed to copy file: {}", e))?;
            tokio::fs::remove_file(&src)
                .await
                .map_err(|e| format!("Failed to remove source after copy: {}", e))?;
        } else {
            return Err(format!("Failed to move file: {}", e));
        }
    }

    DiscoveryTrackRow::update_status(track_id, &DiscoveryStatus::Promoted).await?;

    info!(
        "Promoted discovery track: {} -> {}",
        track.title,
        dest.display()
    );
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
    }

    DiscoveryTrackRow::update_status(track_id, &DiscoveryStatus::Removed).await?;

    info!("Removed discovery track: {}", track.title);
    Ok(())
}
