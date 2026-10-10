use super::{parse, SoundCloudProvider};
use crate::{
    error::{Error, Result},
    library::imports::{ImportProfile, LikesPage},
    models::Availability,
};
use tokio_util::sync::CancellationToken;

pub fn normalize_profile(input: &str) -> Result<String> {
    let input = input.trim();
    let invalid = || Error::Invalid("Enter a SoundCloud username or an HTTPS profile URL".into());
    if input.is_empty() || input.len() > 512 {
        return Err(invalid());
    }
    let username = if input.contains("://") {
        let url = url::Url::parse(input).map_err(|_| invalid())?;
        if url.scheme() != "https"
            || !matches!(
                url.host_str(),
                Some("soundcloud.com" | "www.soundcloud.com")
            )
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some_and(|p| p != 443)
        {
            return Err(invalid());
        }
        url.path().trim_matches('/').to_owned()
    } else {
        input.to_owned()
    };
    if matches!(username.as_str(), "." | "..")
        || username.is_empty()
        || username.len() > 100
        || !username
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        || matches!(
            username.to_lowercase().as_str(),
            "discover"
                | "search"
                | "charts"
                | "you"
                | "stream"
                | "upload"
                | "settings"
                | "terms-of-use"
                | "pages"
        )
    {
        return Err(invalid());
    }
    Ok(format!(
        "https://soundcloud.com/{}",
        username.to_lowercase()
    ))
}
impl SoundCloudProvider {
    pub(super) async fn import_profile(
        &self,
        input: &str,
        cancel: CancellationToken,
    ) -> Result<ImportProfile> {
        let url = normalize_profile(input)?;
        let value = self
            .transport
            .get("/resolve", &[("url", url.clone())], cancel)
            .await?;
        if value["kind"] != "user" {
            return Err(Error::Invalid(
                "This link does not identify a SoundCloud profile".into(),
            ));
        }
        let artist = parse::artist(&value)?;
        Ok(ImportProfile {
            provider: "soundcloud".into(),
            provider_user_id: artist.references[0].provider_id.clone(),
            url: artist.references[0].url.clone().unwrap_or(url),
            name: artist.name,
            artwork: artist.artwork,
        })
    }
    pub(super) async fn import_page(
        &self,
        id: &str,
        cursor: Option<&str>,
        cancel: CancellationToken,
    ) -> Result<LikesPage> {
        let path = format!("/users/{}/track_likes", parse::numeric_id(id, "users")?);
        let value = match cursor {
            None => self.transport.get(&path,&[("limit","100".into()),("linked_partitioning","1".into())],cancel).await,
            Some(cursor) => {
                validate_cursor(cursor,&path)?;
                self.transport.resolve_cancelled(cursor,&[],cancel).await
            }
        }.map_err(|e| if matches!(e,Error::Unavailable) { Error::Invalid("This profile's likes are not publicly accessible. SoundCloud may have hidden them or removed the profile.".into()) } else { e })?;
        parse_likes_response(&value, &path)
    }
}

fn parse_likes_response(value: &serde_json::Value, path: &str) -> Result<LikesPage> {
    let mut page = LikesPage::default();
    for item in parse::collection(value)? {
        let track = if item.get("track").is_some() {
            &item["track"]
        } else {
            item
        };
        page.discovered += 1;
        match parse::track(track) {
            Ok(t) => {
                if t.availability == Availability::Unavailable {
                    page.unavailable += 1;
                }
                page.tracks.push(t);
            }
            Err(_) => page.failed += 1,
        }
    }
    page.next = match value.get("next_href") {
        Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(next)) if !next.is_empty() => {
            validate_cursor(next, path)?;
            Some(next.clone())
        }
        _ => return Err(Error::Malformed),
    };
    Ok(page)
}
fn validate_cursor(raw: &str, path: &str) -> Result<()> {
    let url = url::Url::parse(raw).map_err(|_| Error::Malformed)?;
    if url.scheme() != "https"
        || url.host_str() != Some("api-v2.soundcloud.com")
        || url.path() != path
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|p| p != 443)
        || url.fragment().is_some()
    {
        return Err(Error::Malformed);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn likes_fixture_counts_deleted_and_unavailable_without_claiming_success() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/soundcloud/likes.json"
        ))
        .unwrap();
        let page = parse_likes_response(&value, "/users/1/track_likes").unwrap();
        assert_eq!(page.discovered, 3);
        assert_eq!(page.failed, 1);
        assert_eq!(page.unavailable, 1);
        assert_eq!(page.tracks.len(), 2);
        assert!(page.next.is_none());
        assert!(parse_likes_response(
            &serde_json::json!({"collection":[]}),
            "/users/1/track_likes"
        )
        .is_err());
        assert!(parse_likes_response(
            &serde_json::json!({"collection":[],"next_href":false}),
            "/users/1/track_likes"
        )
        .is_err());
    }
    #[test]
    fn input_normalization() {
        for input in [
            "Creator-123",
            " Creator-123 ",
            "https://soundcloud.com/creator-123/",
            "https://www.soundcloud.com/creator-123?utm_source=share#x",
        ] {
            assert_eq!(
                normalize_profile(input).unwrap(),
                "https://soundcloud.com/creator-123"
            );
        }
        for input in [
            "",
            "https://evil.test/a",
            "http://soundcloud.com/a",
            "https://soundcloud.com/a/track",
            "https://user@soundcloud.com/a",
            "https://soundcloud.com:123/a",
            "https://soundcloud.com/%61",
            "a/b",
            "a b",
            "soundcloud.com/a",
            "../a",
            "https://soundcloud.com/",
        ] {
            assert!(normalize_profile(input).is_err(), "{input}");
        }
    }
    #[test]
    fn cursors_cannot_leave_profile_or_provider() {
        assert!(validate_cursor(
            "https://api-v2.soundcloud.com/users/1/track_likes?cursor=abc",
            "/users/1/track_likes"
        )
        .is_ok());
        for url in [
            "https://evil.test/users/1/track_likes",
            "https://api-v2.soundcloud.com/users/2/track_likes",
            "https://api-v2.soundcloud.com/resolve",
            "http://api-v2.soundcloud.com/users/1/track_likes",
        ] {
            assert!(validate_cursor(url, "/users/1/track_likes").is_err());
        }
    }
}
