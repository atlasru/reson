use crate::{
    error::{Error, Result},
    models::*,
};
use serde_json::Value;
use uuid::Uuid;

pub fn provider_id(value: &Value, kind: &str) -> Result<String> {
    if let Some(urn) = value["urn"].as_str() {
        if urn
            .strip_prefix(&format!("soundcloud:{kind}:"))
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        {
            return Ok(urn.to_owned());
        }
    }
    let id = match &value["id"] {
        Value::Number(n) => n.as_u64().map(|n| n.to_string()),
        Value::String(s) if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) => {
            Some(s.clone())
        }
        _ => None,
    }
    .ok_or(Error::Malformed)?;
    Ok(format!("soundcloud:{kind}:{id}"))
}

pub fn safe_url(value: &Value) -> Option<String> {
    let raw = value.as_str()?;
    let u = url::Url::parse(raw).ok()?;
    let host = u.host_str()?;
    (u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && (host == "soundcloud.com" || host.ends_with(".sndcdn.com")))
    .then(|| raw.to_owned())
}

fn artwork(value: &Value) -> Option<String> {
    safe_url(value).map(|s| s.replace("-large.", "-t500x500."))
}
fn text(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(12000).collect())
}

pub fn artist(v: &Value) -> Result<Artist> {
    let id = provider_id(v, "users")?;
    let name = text(&v["username"]).ok_or(Error::Malformed)?;
    Ok(Artist {
        internal_id: Uuid::new_v4(),
        name,
        artwork: artwork(&v["avatar_url"]),
        description: text(&v["description"]),
        references: vec![ProviderRef {
            provider: "soundcloud".into(),
            provider_id: id,
            url: safe_url(&v["permalink_url"]),
        }],
    })
}

pub fn track(v: &Value) -> Result<Track> {
    let id = provider_id(v, "tracks")?;
    let title = text(&v["title"]).ok_or(Error::Malformed)?;
    let uploader = artist(&v["user"])?;
    let availability = if v["streamable"].as_bool() == Some(false)
        || matches!(v["policy"].as_str(), Some("BLOCK"))
        || v["access"] == "blocked"
    {
        Availability::Unavailable
    } else if v["policy"] == "SNIP" || v["access"] == "preview" {
        Availability::Preview
    } else {
        Availability::Playable
    };
    let duration = v["full_duration"]
        .as_u64()
        .or_else(|| v["duration"].as_u64())
        .ok_or(Error::Malformed)?;
    if duration > 86_400_000 {
        return Err(Error::Malformed);
    }
    let art = artwork(&v["artwork_url"]).or_else(|| uploader.artwork.clone());
    let explicit = v["publisher_metadata"]["explicit"].as_bool();
    Ok(Track {
        internal_id: Uuid::new_v4(),
        title,
        artists: vec![uploader],
        album: text(&v["publisher_metadata"]["album_title"]),
        artwork: art,
        duration_ms: duration,
        explicit,
        availability,
        sources: vec![TrackSource {
            provider: "soundcloud".into(),
            provider_id: id,
            availability,
            url: safe_url(&v["permalink_url"]),
        }],
    })
}

pub fn playlist(v: &Value) -> Result<Playlist> {
    let id = provider_id(v, "playlists")?;
    Ok(Playlist {
        internal_id: Uuid::new_v4(),
        title: text(&v["title"]).ok_or(Error::Malformed)?,
        artwork: artwork(&v["artwork_url"]),
        description: text(&v["description"]),
        owner: artist(&v["user"]).ok(),
        tracks: v["tracks"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| track(v).ok()).collect())
            .unwrap_or_default(),
        track_count: v["track_count"].as_u64().unwrap_or(0).min(10000) as usize,
        reference: Some(ProviderRef {
            provider: "soundcloud".into(),
            provider_id: id,
            url: safe_url(&v["permalink_url"]),
        }),
    })
}

pub fn collection(value: &Value) -> Result<&Vec<Value>> {
    value["collection"]
        .as_array()
        .or_else(|| value.as_array())
        .ok_or(Error::Malformed)
}

pub fn numeric_id(id: &str, kind: &str) -> Result<String> {
    let s = id
        .strip_prefix(&format!("soundcloud:{kind}:"))
        .unwrap_or(id);
    if s.is_empty() || s.len() > 24 || !s.bytes().all(|c| c.is_ascii_digit()) {
        return Err(Error::Invalid("Invalid provider reference".into()));
    }
    Ok(s.to_owned())
}
