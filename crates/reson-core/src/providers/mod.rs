pub mod soundcloud;

use crate::{
    error::{Error, Result},
    models::*,
};
use async_trait::async_trait;
use serde::Serialize;
use std::{collections::HashMap, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Search,
    Playback,
    Artists,
    Playlists,
    Related,
    Authentication,
    Likes,
    LibraryWrite,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub capabilities: Vec<Capability>,
    pub mode: String,
}

#[async_trait]
pub trait MusicProvider: Send + Sync {
    fn info(&self) -> ProviderInfo;
    async fn search(
        &self,
        query: &str,
        offset: u32,
        cancel: CancellationToken,
    ) -> Result<SearchResults>;
    async fn track(&self, id: &str) -> Result<Track>;
    async fn resolve_stream(&self, source: &TrackSource) -> Result<StreamSource>;
    async fn artist(&self, _id: &str, _offset: u32) -> Result<ArtistPage> {
        Err(Error::Unsupported("artists"))
    }
    async fn playlist(&self, _id: &str) -> Result<Playlist> {
        Err(Error::Unsupported("playlists"))
    }
    async fn related(&self, _id: &str) -> Result<Vec<Track>> {
        Err(Error::Unsupported("related tracks"))
    }
    async fn discover(&self) -> Result<Vec<Track>> {
        Err(Error::Unsupported("discovery"))
    }
    async fn resolve_url(&self, _url: &str) -> Result<ResolvedEntity> {
        Err(Error::Unsupported("URL resolution"))
    }
}

#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn MusicProvider>>,
}
impl ProviderRegistry {
    pub fn register(&mut self, provider: Arc<dyn MusicProvider>) {
        self.providers.insert(provider.info().id, provider);
    }
    pub fn get(&self, id: &str) -> Result<Arc<dyn MusicProvider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| Error::Invalid("Unknown music provider".into()))
    }
    pub fn list(&self) -> Vec<ProviderInfo> {
        self.providers.values().map(|p| p.info()).collect()
    }
}
