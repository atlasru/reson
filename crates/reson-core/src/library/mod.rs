pub mod imports;
use crate::{
    error::Result,
    models::{Playlist, Track},
    storage::Storage,
};
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct Library {
    pub favorites: Vec<Track>,
    pub recent: Vec<Track>,
    pub playlists: Vec<Playlist>,
    pub local_favorite_ids: Vec<uuid::Uuid>,
    pub import_sources: Vec<imports::ImportSource>,
    pub import_track_sources: std::collections::HashMap<uuid::Uuid, Vec<uuid::Uuid>>,
}
impl Library {
    pub fn load(storage: &Storage) -> Result<Self> {
        Ok(Self {
            favorites: storage.favorites()?,
            recent: storage.history()?,
            playlists: storage.playlists()?,
            local_favorite_ids: storage.local_favorite_ids()?,
            import_sources: storage.import_sources()?,
            import_track_sources: storage.import_track_sources()?,
        })
    }
}
