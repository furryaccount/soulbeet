use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatedSong {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_id: Option<String>,
    pub genre: Option<String>,
    pub duration: Option<u32>,
    pub path: Option<String>,
    pub user_rating: Option<u8>,
    pub average_rating: Option<f64>,
    pub play_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletionReview {
    pub id: String,
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: Option<String>,
    pub rating: Option<u8>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LibraryStats {
    pub total_tracks: u32,
    pub rated_tracks: u32,
    pub unrated_tracks: u32,
    pub average_rating: f64,
    pub rating_distribution: [u32; 5],
    pub total_albums: u32,
    pub total_artists: u32,
    pub genres: Vec<(String, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryTrack {
    pub id: String,
    pub song_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub path: String,
    pub folder_id: String,
    pub profile: String,
    pub rating: Option<u8>,
    pub status: DiscoveryStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiscoveryStatus {
    Pending,
    Promoted,
    Removed,
}

impl std::fmt::Display for DiscoveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryStatus::Pending => write!(f, "Pending"),
            DiscoveryStatus::Promoted => write!(f, "Promoted"),
            DiscoveryStatus::Removed => write!(f, "Removed"),
        }
    }
}

impl std::str::FromStr for DiscoveryStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Pending" => Ok(DiscoveryStatus::Pending),
            "Promoted" => Ok(DiscoveryStatus::Promoted),
            "Removed" => Ok(DiscoveryStatus::Removed),
            _ => Err(format!("Unknown discovery status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryConfig {
    pub enabled: bool,
    pub folder_id: Option<String>,
    pub folder_name: Option<String>,
    pub track_count: u32,
    pub lifetime_days: u32,
    pub profiles: String,
    pub playlist_names: std::collections::HashMap<String, String>,
    pub navidrome_playlist_ids: std::collections::HashMap<String, String>,
    pub last_generated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncResult {
    pub deleted_tracks: u32,
    pub promoted_tracks: u32,
    pub removed_tracks: u32,
    pub total_songs_scanned: u32,
}
