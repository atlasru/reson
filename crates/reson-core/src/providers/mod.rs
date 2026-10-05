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
    Discovery,
    UrlResolution,
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
    fn artwork_hosts(&self) -> &'static [&'static str] {
        &[]
    }
    fn external_hosts(&self) -> &'static [&'static str] {
        &[]
    }
    async fn search(
        &self,
        _query: &str,
        _offset: u32,
        _cancel: CancellationToken,
    ) -> Result<SearchResults> {
        Err(Error::Unsupported("search"))
    }
    async fn track(&self, _id: &str) -> Result<Track> {
        Err(Error::Unsupported("track metadata"))
    }
    async fn resolve_stream(&self, _source: &TrackSource) -> Result<StreamSource> {
        Err(Error::Unsupported("playback"))
    }
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
        let mut providers = self
            .providers
            .values()
            .map(|p| p.info())
            .collect::<Vec<_>>();
        providers.sort_by(|a, b| a.id.cmp(&b.id));
        providers
    }
    pub fn artwork_hosts(&self) -> Vec<String> {
        let mut hosts = self
            .providers
            .values()
            .flat_map(|p| p.artwork_hosts().iter().map(|h| (*h).to_owned()))
            .collect::<Vec<_>>();
        hosts.sort();
        hosts.dedup();
        hosts
    }
    pub fn allows_external_url(&self, url: &url::Url) -> bool {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none_or(|p| p == 443)
            && url.host_str().is_some_and(|host| {
                self.providers.values().any(|p| {
                    p.external_hosts()
                        .iter()
                        .any(|h| host == *h || host == format!("www.{h}"))
                })
            })
    }
}
