use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    #[default]
    Playable,
    Preview,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRef {
    pub provider: String,
    pub provider_id: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub internal_id: Uuid,
    pub name: String,
    pub artwork: Option<String>,
    pub description: Option<String>,
    pub references: Vec<ProviderRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSource {
    pub provider: String,
    pub provider_id: String,
    pub availability: Availability,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub internal_id: Uuid,
    pub title: String,
    pub artists: Vec<Artist>,
    pub album: Option<String>,
    pub artwork: Option<String>,
    pub duration_ms: u64,
    pub explicit: Option<bool>,
    pub availability: Availability,
    pub sources: Vec<TrackSource>,
}

impl Track {
    pub fn artist_name(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub internal_id: Uuid,
    pub title: String,
    pub artwork: Option<String>,
    pub description: Option<String>,
    pub owner: Option<Artist>,
    pub tracks: Vec<Track>,
    pub track_count: usize,
    pub reference: Option<ProviderRef>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResults {
    pub tracks: Vec<Track>,
    pub artists: Vec<Artist>,
    pub playlists: Vec<Playlist>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistPage {
    pub artist: Artist,
    pub tracks: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Hls,
    Progressive,
}

// Stream URLs are transient, private to the core, and never written to SQLite.
#[derive(Clone)]
pub struct StreamSource {
    pub url: String,
    pub kind: StreamKind,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "entity", rename_all = "snake_case")]
pub enum ResolvedEntity {
    Track(Track),
    Artist(Artist),
    Playlist(Playlist),
}
