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
}
impl Library {
    pub fn load(storage: &Storage) -> Result<Self> {
        Ok(Self {
            favorites: storage.favorites()?,
            recent: storage.history()?,
            playlists: storage.playlists()?,
        })
    }
}
