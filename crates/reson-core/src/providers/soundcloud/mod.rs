pub mod parse;
mod transport;

use crate::{
    error::{Error, Result},
    models::*,
    providers::{Capability, MusicProvider, ProviderInfo},
};
use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use transport::Transport;

pub struct SoundCloudProvider {
    transport: Transport,
}
impl SoundCloudProvider {
    pub fn new() -> Result<Self> {
        Ok(Self {
            transport: Transport::new()?,
        })
    }
    async fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        self.transport
            .get(path, params, CancellationToken::new())
            .await
    }
    async fn raw_track(&self, id: &str) -> Result<Value> {
        self.get(
            &format!("/tracks/{}", parse::numeric_id(id, "tracks")?),
            &[],
        )
        .await
    }
    async fn hydrate_playlist(&self, v: Value) -> Result<Playlist> {
        let mut playlist = parse::playlist(&v)?;
        if let Some(items) = v["tracks"].as_array() {
            let missing = items
                .iter()
                .filter(|t| t["title"].as_str().is_none())
                .filter_map(|t| parse::provider_id(t, "tracks").ok())
                .collect::<Vec<_>>();
            let mut hydrated = std::collections::HashMap::new();
            for chunk in missing.chunks(50) {
                let ids = chunk
                    .iter()
                    .filter_map(|s| parse::numeric_id(s, "tracks").ok())
                    .collect::<Vec<_>>()
                    .join(",");
                let response = self.get("/tracks", &[("ids", ids)]).await?;
                for item in parse::collection(&response)? {
                    if let Ok(track) = parse::track(item) {
                        hydrated.insert(track.sources[0].provider_id.clone(), track);
                    }
                }
            }
            playlist.tracks = items
                .iter()
                .filter_map(|v| {
                    parse::track(v).ok().or_else(|| {
                        parse::provider_id(v, "tracks")
                            .ok()
                            .and_then(|id| hydrated.remove(&id))
                    })
                })
                .collect();
        }
        if playlist.artwork.is_none() {
            playlist.artwork = playlist.tracks.first().and_then(|t| t.artwork.clone());
        }
        Ok(playlist)
    }
}

#[async_trait]
impl MusicProvider for SoundCloudProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            id: "soundcloud".into(),
            display_name: "SoundCloud".into(),
            mode: "public_guest".into(),
            capabilities: vec![
                Capability::Search,
                Capability::Playback,
                Capability::Artists,
                Capability::Playlists,
                Capability::Related,
            ],
        }
    }
    async fn search(
        &self,
        query: &str,
        offset: u32,
        cancel: CancellationToken,
    ) -> Result<SearchResults> {
        let query = query.trim();
        if query.is_empty() || query.len() > 500 || offset > 10000 {
            return Err(Error::Invalid(
                "Search must contain 1–500 characters".into(),
            ));
        }
        let params = [
            ("q", query.to_owned()),
            ("limit", "30".into()),
            ("offset", offset.to_string()),
        ];
        let (tracks, artists, playlists) = tokio::try_join!(
            self.transport
                .get("/search/tracks", &params, cancel.clone()),
            self.transport.get("/search/users", &params, cancel.clone()),
            self.transport
                .get("/search/playlists", &params, cancel.clone()),
        )?;
        Ok(SearchResults {
            tracks: parse::collection(&tracks)?
                .iter()
                .filter_map(|v| parse::track(v).ok())
                .collect(),
            artists: parse::collection(&artists)?
                .iter()
                .filter_map(|v| parse::artist(v).ok())
                .collect(),
            playlists: parse::collection(&playlists)?
                .iter()
                .filter_map(|v| parse::playlist(v).ok())
                .collect(),
            has_more: tracks["next_href"].is_string(),
        })
    }
    async fn track(&self, id: &str) -> Result<Track> {
        parse::track(&self.raw_track(id).await?)
    }
    async fn resolve_stream(&self, source: &TrackSource) -> Result<StreamSource> {
        // Refresh metadata and authorization for every playback/retry. Never persist either.
        let v = self.raw_track(&source.provider_id).await?;
        let track = parse::track(&v)?;
        if track.availability == Availability::Unavailable {
            return Err(Error::Unavailable);
        }
        let items = v["media"]["transcodings"]
            .as_array()
            .ok_or(Error::Unavailable)?;
        let mut streams = items
            .iter()
            .filter(|s| s["url"].is_string())
            .collect::<Vec<_>>();
        streams.sort_by_key(|s| match s["preset"].as_str().unwrap_or("") {
            "aac_160k" => 0,
            "aac_96k" => 1,
            _ if s["format"]["protocol"] == "hls" => 2,
            _ => 3,
        });
        for s in streams {
            let Some(url) = s["url"].as_str() else {
                continue;
            };
            let params = v["track_authorization"]
                .as_str()
                .map(|a| vec![("track_authorization", a.to_owned())])
                .unwrap_or_default();
            match self.transport.resolve(url, &params).await {
                Ok(response) => {
                    let url = response["url"].as_str().ok_or(Error::Malformed)?.to_owned();
                    transport::validate_stream_url(&url)?;
                    let preview = s["snipped"].as_bool().unwrap_or(false)
                        || track.availability == Availability::Preview;
                    return Ok(StreamSource {
                        url,
                        kind: if s["format"]["protocol"] == "hls" {
                            StreamKind::Hls
                        } else {
                            StreamKind::Progressive
                        },
                        duration_ms: if preview {
                            s["duration"].as_u64().unwrap_or(30000)
                        } else {
                            track.duration_ms
                        },
                    });
                }
                Err(Error::Unavailable) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(Error::Unavailable)
    }
    async fn artist(&self, id: &str, offset: u32) -> Result<ArtistPage> {
        let id = parse::numeric_id(id, "users")?;
        let params = [
            ("limit", "50".into()),
            ("offset", offset.min(10000).to_string()),
        ];
        let user_path = format!("/users/{id}");
        let tracks_path = format!("/users/{id}/tracks");
        let playlists_path = format!("/users/{id}/playlists");
        let (user, tracks, playlists) = tokio::try_join!(
            self.get(&user_path, &[]),
            self.get(&tracks_path, &params),
            self.get(&playlists_path, &params)
        )?;
        Ok(ArtistPage {
            artist: parse::artist(&user)?,
            tracks: parse::collection(&tracks)?
                .iter()
                .filter_map(|v| parse::track(v).ok())
                .collect(),
            playlists: parse::collection(&playlists)?
                .iter()
                .filter_map(|v| parse::playlist(v).ok())
                .collect(),
            has_more: tracks["next_href"].is_string(),
        })
    }
    async fn playlist(&self, id: &str) -> Result<Playlist> {
        self.hydrate_playlist(
            self.get(
                &format!("/playlists/{}", parse::numeric_id(id, "playlists")?),
                &[],
            )
            .await?,
        )
        .await
    }
    async fn related(&self, id: &str) -> Result<Vec<Track>> {
        let v = self
            .get(
                &format!("/tracks/{}/related", parse::numeric_id(id, "tracks")?),
                &[("limit", "30".into())],
            )
            .await?;
        Ok(parse::collection(&v)?
            .iter()
            .filter_map(|v| parse::track(v).ok())
            .collect())
    }
    async fn discover(&self) -> Result<Vec<Track>> {
        let v = self
            .get(
                "/charts",
                &[
                    ("kind", "trending".into()),
                    ("genre", "soundcloud:genres:all-music".into()),
                    ("limit", "30".into()),
                ],
            )
            .await?;
        Ok(parse::collection(&v)?
            .iter()
            .filter_map(|v| parse::track(&v["track"]).ok())
            .collect())
    }
    async fn resolve_url(&self, raw: &str) -> Result<ResolvedEntity> {
        let u = url::Url::parse(raw)
            .map_err(|_| Error::Invalid("Use a SoundCloud HTTPS link".into()))?;
        if u.scheme() != "https"
            || !matches!(u.host_str(), Some("soundcloud.com" | "www.soundcloud.com"))
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err(Error::Invalid("Use a SoundCloud HTTPS link".into()));
        }
        let v = self.get("/resolve", &[("url", raw.into())]).await?;
        match v["kind"].as_str() {
            Some("track") => Ok(ResolvedEntity::Track(parse::track(&v)?)),
            Some("user") => Ok(ResolvedEntity::Artist(parse::artist(&v)?)),
            Some("playlist") => Ok(ResolvedEntity::Playlist(self.hydrate_playlist(v).await?)),
            _ => Err(Error::Malformed),
        }
    }
}
