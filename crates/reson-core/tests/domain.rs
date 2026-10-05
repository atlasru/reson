use reson_core::{
    config::Settings,
    models::*,
    providers::{soundcloud::parse, Capability, ProviderRegistry},
    queue::{Queue, RepeatMode},
    storage::Storage,
};
use serde_json::json;
use uuid::Uuid;
fn fixture() -> serde_json::Value {
    json!({"urn":"soundcloud:tracks:4294967298","title":"A real-sized identifier","duration":180000,"streamable":true,"policy":"ALLOW","artwork_url":"https://i1.sndcdn.com/art-large.jpg","publisher_metadata":{"explicit":false},"user":{"id":7000000000u64,"username":"Artist","avatar_url":null,"permalink_url":"https://soundcloud.com/artist"},"permalink_url":"https://soundcloud.com/artist/track"})
}
#[test]
fn normalized_entities_do_not_use_provider_ids_as_primary_keys() {
    let t = parse::track(&fixture()).unwrap();
    assert_eq!(t.sources[0].provider_id, "soundcloud:tracks:4294967298");
    assert_eq!(
        t.artists[0].references[0].provider_id,
        "soundcloud:users:7000000000"
    );
    assert_eq!(t.internal_id.get_version_num(), 4);
    assert_eq!(
        t.artwork.as_deref(),
        Some("https://i1.sndcdn.com/art-t500x500.jpg")
    );
    assert_eq!(t.explicit, Some(false));
}
#[test]
fn missing_untrusted_or_malformed_responses_are_rejected() {
    for bad in [
        json!(null),
        json!({}),
        json!({"id":"https://evil.test"}),
        json!({"title":false,"id":1}),
    ] {
        assert!(parse::track(&bad).is_err());
    }
    assert!(parse::collection(&json!({"collection":null})).is_err());
    assert!(parse::numeric_id("../secrets", "tracks").is_err());
    assert!(parse::safe_url(&json!("https://i1.sndcdn.com.evil.test/img")).is_none());
    assert!(parse::safe_url(&json!("javascript:alert(1)")).is_none());
}
#[test]
fn unavailable_and_preview_are_distinct() {
    let mut v = fixture();
    v["policy"] = json!("SNIP");
    assert_eq!(
        parse::track(&v).unwrap().availability,
        Availability::Preview
    );
    v["policy"] = json!("BLOCK");
    assert_eq!(
        parse::track(&v).unwrap().availability,
        Availability::Unavailable
    );
    v["policy"] = json!("ALLOW");
    v["streamable"] = json!(false);
    assert_eq!(
        parse::track(&v).unwrap().availability,
        Availability::Unavailable
    );
}
#[test]
fn migration_identity_and_user_state_survive_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("reson.sqlite");
    let s = Storage::open(&path).unwrap();
    let first = s
        .intern_tracks(vec![parse::track(&fixture()).unwrap()])
        .unwrap()
        .remove(0);
    let duplicate = s
        .intern_tracks(vec![parse::track(&fixture()).unwrap()])
        .unwrap()
        .remove(0);
    assert_eq!(first.internal_id, duplicate.internal_id);
    assert_eq!(
        first.artists[0].internal_id,
        duplicate.artists[0].internal_id
    );
    let installation = s.installation().unwrap().installation_id;
    s.favorite(first.internal_id, true).unwrap();
    s.record_play(first.internal_id).unwrap();
    let playlist = s.create_playlist("Saved music").unwrap();
    s.add_to_playlist(playlist, &[first.internal_id, first.internal_id])
        .unwrap();
    s.remove_from_playlist(playlist, 0).unwrap();
    let mut q = Queue::default();
    q.replace(vec![first.clone(), duplicate], 1).unwrap();
    q.repeat = RepeatMode::Queue;
    q.set_shuffle(true);
    s.save_queue(&q, 32100).unwrap();
    let settings = Settings {
        volume: 0.42,
        ..Default::default()
    };
    s.save_settings(&settings).unwrap();
    drop(s);
    let s = Storage::open(&path).unwrap();
    assert_eq!(s.installation().unwrap().installation_id, installation);
    assert_eq!(s.favorites().unwrap()[0].internal_id, first.internal_id);
    assert_eq!(s.history().unwrap().len(), 1);
    assert_eq!(s.playlists().unwrap()[0].tracks.len(), 1);
    let (saved, pos) = s.load_queue().unwrap();
    assert_eq!(saved.current, q.current);
    assert_eq!(saved.order, q.order);
    assert_eq!(pos, 32100);
    assert_eq!(s.settings().unwrap().volume, 0.42);
    s.cleanup_metadata().unwrap();
    assert!(s.track(first.internal_id).is_ok());
    s.delete_playlist(playlist).unwrap();
    assert!(s.playlists().unwrap().is_empty());
}
#[test]
fn bounded_queue_repairs_corrupt_order_and_duplicate_entries() {
    let mut q = Queue::default();
    q.replace(vec![parse::track(&fixture()).unwrap()], 0)
        .unwrap();
    q.order = vec![Uuid::new_v4(), q.current.unwrap(), q.current.unwrap()];
    q.entries.push(q.entries[0].clone());
    q.validate();
    assert_eq!(q.order.len(), 1);
    assert_eq!(q.entries.len(), 1);
    assert_eq!(q.advance(false), None);
}
#[test]
fn registry_exposes_capabilities_without_forcing_authentication() {
    let mut r = ProviderRegistry::default();
    r.register(std::sync::Arc::new(
        reson_core::providers::soundcloud::SoundCloudProvider::new().unwrap(),
    ));
    let provider = r.get("soundcloud").unwrap();
    assert!(provider.info().capabilities.contains(&Capability::Playback));
    assert!(!provider
        .info()
        .capabilities
        .contains(&Capability::Authentication));
    assert!(r.get("missing").is_err());
}
