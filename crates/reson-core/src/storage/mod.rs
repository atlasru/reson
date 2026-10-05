use crate::{
    config::Settings,
    error::{Error, Result},
    models::*,
    queue::Queue,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct Storage(Arc<Mutex<Connection>>);
#[derive(Debug, Clone, Serialize)]
pub struct Installation {
    pub installation_id: String,
    pub platform: String,
    pub app_version: String,
    pub first_seen: i64,
    pub last_seen: i64,
}
impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;",
        )?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(Error::Invalid(
                "This database requires a newer version of Reson".into(),
            ));
        }
        if version == 0 {
            let tx = conn.transaction()?;
            tx.execute_batch(include_str!("../../../../migrations/001_init.sql"))?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }
        let time = now();
        conn.execute("INSERT INTO installation VALUES(1,?1,?2,?3,?4,?4) ON CONFLICT(singleton) DO UPDATE SET app_version=excluded.app_version,last_seen=excluded.last_seen",params![Uuid::new_v4().to_string(),std::env::consts::OS,env!("CARGO_PKG_VERSION"),time])?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }
    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.0
            .lock()
            .map_err(|_| Error::Invalid("Storage is busy after an internal failure".into()))
    }
    pub fn intern_tracks(&self, tracks: Vec<Track>) -> Result<Vec<Track>> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let tracks = tracks
            .into_iter()
            .map(|t| intern_track(&tx, t))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        drop(conn);
        self.cleanup_metadata()?;
        Ok(tracks)
    }
    pub fn intern_artists(&self, artists: Vec<Artist>) -> Result<Vec<Artist>> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let artists = artists
            .into_iter()
            .map(|a| intern_artist(&tx, a))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        drop(conn);
        self.cleanup_metadata()?;
        Ok(artists)
    }
    pub fn track(&self, id: Uuid) -> Result<Track> {
        let json: String = self.connection()?.query_row(
            "SELECT json FROM tracks WHERE id=?1",
            [id.to_string()],
            |r| r.get(0),
        )?;
        Ok(serde_json::from_str(&json)?)
    }
    pub fn save_queue(&self, queue: &Queue, position: u64) -> Result<()> {
        self.connection()?.execute("INSERT INTO queue VALUES(1,?1,?2) ON CONFLICT(singleton) DO UPDATE SET json=excluded.json,position_ms=excluded.position_ms",params![serde_json::to_string(queue)?,position.min(i64::MAX as u64) as i64])?;
        Ok(())
    }
    pub fn load_queue(&self) -> Result<(Queue, u64)> {
        let data: Option<(String, i64)> = self
            .connection()?
            .query_row(
                "SELECT json,position_ms FROM queue WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((json, pos)) = data {
            let mut q: Queue = serde_json::from_str(&json)?;
            q.validate();
            Ok((q, pos.max(0) as u64))
        } else {
            Ok((Queue::default(), 0))
        }
    }
    pub fn settings(&self) -> Result<Settings> {
        let json: Option<String> = self
            .connection()?
            .query_row("SELECT json FROM settings WHERE key='app'", [], |r| {
                r.get(0)
            })
            .optional()?;
        let settings = json
            .map(|s| serde_json::from_str::<Settings>(&s))
            .transpose()?
            .unwrap_or_default();
        settings.validate()?;
        Ok(settings)
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        self.connection()?.execute("INSERT INTO settings VALUES('app',?1) ON CONFLICT(key) DO UPDATE SET json=excluded.json",[serde_json::to_string(settings)?])?;
        Ok(())
    }
    pub fn favorite(&self, id: Uuid, enabled: bool) -> Result<()> {
        if enabled {
            self.connection()?.execute(
                "INSERT OR IGNORE INTO favorites VALUES(?1,?2)",
                params![id.to_string(), now()],
            )?;
        } else {
            self.connection()?
                .execute("DELETE FROM favorites WHERE track_id=?1", [id.to_string()])?;
        }
        Ok(())
    }
    pub fn favorites(&self) -> Result<Vec<Track>> {
        self.list("SELECT t.json FROM favorites f JOIN tracks t ON t.id=f.track_id ORDER BY f.added_at DESC")
    }
    pub fn history(&self) -> Result<Vec<Track>> {
        self.list("SELECT t.json FROM tracks t JOIN (SELECT track_id,MAX(id) AS recent FROM history GROUP BY track_id) h ON h.track_id=t.id ORDER BY h.recent DESC LIMIT 100")
    }
    fn list(&self, sql: &str) -> Result<Vec<Track>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn record_play(&self, id: Uuid) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO history(track_id,played_at) VALUES(?1,?2)",
            params![id.to_string(), now()],
        )?;
        tx.execute("DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY id DESC LIMIT 1000)",[])?;
        tx.commit()?;
        Ok(())
    }
    pub fn create_playlist(&self, title: &str) -> Result<Uuid> {
        let title = valid_title(title)?;
        let id = Uuid::new_v4();
        self.connection()?.execute(
            "INSERT INTO playlists VALUES(?1,?2,NULL,?3)",
            params![id.to_string(), title, now()],
        )?;
        Ok(id)
    }
    pub fn rename_playlist(&self, id: Uuid, title: &str) -> Result<()> {
        let title = valid_title(title)?;
        self.connection()?.execute(
            "UPDATE playlists SET title=?2 WHERE id=?1",
            params![id.to_string(), title],
        )?;
        Ok(())
    }
    pub fn delete_playlist(&self, id: Uuid) -> Result<()> {
        self.connection()?
            .execute("DELETE FROM playlists WHERE id=?1", [id.to_string()])?;
        Ok(())
    }
    pub fn add_to_playlist(&self, id: Uuid, tracks: &[Uuid]) -> Result<()> {
        if tracks.len() > 5000 {
            return Err(Error::Invalid("Playlist limit is 5000 tracks".into()));
        }
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM playlist_tracks WHERE playlist_id=?1",
            [id.to_string()],
            |r| r.get(0),
        )?;
        if count + tracks.len() as i64 > 5000 {
            return Err(Error::Invalid("Playlist limit is 5000 tracks".into()));
        }
        for (i, track) in tracks.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks VALUES(?1,?2,?3)",
                params![id.to_string(), count + i as i64, track.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn remove_from_playlist(&self, id: Uuid, position: usize) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id=?1 AND position=?2",
            params![id.to_string(), position as i64],
        )?;
        // Rebuild positions rather than updating into an occupied unique key.
        let ids = {
            let mut stmt = tx.prepare(
                "SELECT track_id FROM playlist_tracks WHERE playlist_id=?1 ORDER BY position",
            )?;
            let rows = stmt.query_map([id.to_string()], |r| r.get::<_, String>(0))?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id=?1",
            [id.to_string()],
        )?;
        for (i, track) in ids.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks VALUES(?1,?2,?3)",
                params![id.to_string(), i as i64, track],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn playlists(&self) -> Result<Vec<Playlist>> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT id,title,artwork FROM playlists ORDER BY created_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        let mut out = vec![];
        for row in rows {
            let (id, title, artwork) = row?;
            let mut tracks_stmt=conn.prepare("SELECT t.json FROM playlist_tracks pt JOIN tracks t ON t.id=pt.track_id WHERE pt.playlist_id=?1 ORDER BY pt.position")?;
            let tracks = tracks_stmt
                .query_map([&id], |r| r.get::<_, String>(0))?
                .map(|r| Ok(serde_json::from_str(&r?)?))
                .collect::<Result<Vec<Track>>>()?;
            out.push(Playlist {
                internal_id: Uuid::parse_str(&id).map_err(|_| Error::Malformed)?,
                title,
                artwork: artwork.or_else(|| tracks.first().and_then(|t| t.artwork.clone())),
                description: None,
                owner: None,
                track_count: tracks.len(),
                tracks,
                reference: None,
            });
        }
        Ok(out)
    }
    pub fn installation(&self) -> Result<Installation> {
        Ok(self.connection()?.query_row("SELECT installation_id,platform,app_version,first_seen,last_seen FROM installation WHERE singleton=1",[],|r|Ok(Installation{installation_id:r.get(0)?,platform:r.get(1)?,app_version:r.get(2)?,first_seen:r.get(3)?,last_seen:r.get(4)?}))?)
    }
    pub fn cleanup_metadata(&self) -> Result<()> {
        // Retain entities referenced by user data; cap the remaining normalized metadata.
        self.connection()?.execute("DELETE FROM tracks WHERE id NOT IN (SELECT track_id FROM favorites UNION SELECT track_id FROM history UNION SELECT track_id FROM playlist_tracks) AND id NOT IN (SELECT id FROM tracks ORDER BY cached_at DESC LIMIT 10000) AND id NOT IN (SELECT json_extract(j.value,'$.track.internal_id') FROM json_each((SELECT json FROM queue WHERE singleton=1),'$.entries') j)",[])?;
        self.connection()?.execute("DELETE FROM artists WHERE id NOT IN (SELECT id FROM artists ORDER BY rowid DESC LIMIT 10000) AND id NOT IN (SELECT json_extract(a.value,'$.internal_id') FROM tracks t,json_each(t.json,'$.artists') a)",[])?;
        Ok(())
    }
}
fn intern_artist(tx: &Transaction<'_>, mut artist: Artist) -> Result<Artist> {
    for source in &artist.references {
        let id: Option<String> = tx
            .query_row(
                "SELECT artist_id FROM artist_sources WHERE provider=?1 AND provider_id=?2",
                params![source.provider, source.provider_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            artist.internal_id = Uuid::parse_str(&id).map_err(|_| Error::Malformed)?;
            break;
        }
    }
    tx.execute(
        "INSERT INTO artists VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET json=excluded.json",
        params![
            artist.internal_id.to_string(),
            serde_json::to_string(&artist)?
        ],
    )?;
    for source in &artist.references {
        tx.execute("INSERT INTO artist_sources VALUES(?1,?2,?3) ON CONFLICT(provider,provider_id) DO UPDATE SET artist_id=excluded.artist_id",params![source.provider,source.provider_id,artist.internal_id.to_string()])?;
    }
    Ok(artist)
}
fn intern_track(tx: &Transaction<'_>, mut track: Track) -> Result<Track> {
    for source in &track.sources {
        let id: Option<String> = tx
            .query_row(
                "SELECT track_id FROM track_sources WHERE provider=?1 AND provider_id=?2",
                params![source.provider, source.provider_id],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = id {
            track.internal_id = Uuid::parse_str(&id).map_err(|_| Error::Malformed)?;
            break;
        }
    }
    let previous: Option<String> = tx
        .query_row(
            "SELECT json FROM tracks WHERE id=?1",
            [track.internal_id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(previous) = previous {
        let previous: Track = serde_json::from_str(&previous)?;
        for source in previous.sources {
            if !track
                .sources
                .iter()
                .any(|s| s.provider == source.provider && s.provider_id == source.provider_id)
            {
                track.sources.push(source);
            }
        }
    }
    track.artists = track
        .artists
        .into_iter()
        .map(|a| intern_artist(tx, a))
        .collect::<Result<_>>()?;
    tx.execute("INSERT INTO tracks VALUES(?1,?2,?3) ON CONFLICT(id) DO UPDATE SET json=excluded.json,cached_at=excluded.cached_at",params![track.internal_id.to_string(),serde_json::to_string(&track)?,now()])?;
    for source in &track.sources {
        tx.execute("INSERT INTO track_sources VALUES(?1,?2,?3,?4,?5) ON CONFLICT(provider,provider_id) DO UPDATE SET availability=excluded.availability,url=excluded.url",params![source.provider,source.provider_id,track.internal_id.to_string(),serde_json::to_string(&source.availability)?,source.url])?;
    }
    Ok(track)
}
fn valid_title(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 120 {
        return Err(Error::Invalid(
            "Playlist name must be 1–120 characters".into(),
        ));
    }
    Ok(title)
}
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}
