use super::{intern_track, now, Storage};
use crate::{
    error::{Error, Result},
    library::imports::{ImportProfile, ImportProgress, ImportSource, ImportStatus},
    models::Track,
};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use uuid::Uuid;

impl Storage {
    pub fn begin_import(&self, profile: &ImportProfile, job: Uuid) -> Result<Uuid> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let existing: Option<(String, Option<String>)> = tx.query_row("SELECT id,active_job FROM import_sources WHERE provider=?1 AND provider_user_id=?2", params![profile.provider, profile.provider_user_id], |r| Ok((r.get(0)?, r.get(1)?))).optional()?;
        let id = match existing {
            Some((_, Some(_))) => {
                return Err(Error::Invalid(
                    "This profile is already being imported".into(),
                ))
            }
            Some((id, None)) => Uuid::parse_str(&id).map_err(|_| Error::Malformed)?,
            None => Uuid::new_v4(),
        };
        tx.execute("INSERT INTO import_sources(id,provider,provider_user_id,url,name,artwork,created_at,active_job) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(provider,provider_user_id) DO UPDATE SET url=excluded.url,name=excluded.name,artwork=excluded.artwork,active_job=excluded.active_job", params![id.to_string(), profile.provider, profile.provider_user_id, profile.url, profile.name, profile.artwork, now(), job.to_string()])?;
        tx.commit()?;
        Ok(id)
    }
    pub fn save_import_page(
        &self,
        source: Uuid,
        job: Uuid,
        tracks: Vec<Track>,
    ) -> Result<(usize, usize)> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let owned: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM import_sources WHERE id=?1 AND active_job=?2)",
            params![source.to_string(), job.to_string()],
            |r| r.get(0),
        )?;
        if !owned {
            return Err(Error::Cancelled);
        }
        let mut saved = 0;
        let mut duplicates = 0;
        for track in tracks {
            if track.sources.is_empty() {
                return Err(Error::Malformed);
            }
            let track = intern_track(&tx, track)?;
            let id = track.internal_id.to_string();
            let dismissed: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM import_dismissals WHERE source_id=?1 AND track_id=?2)",
                params![source.to_string(), id],
                |r| r.get(0),
            )?;
            if dismissed {
                duplicates += 1;
                continue;
            }
            let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM favorites WHERE track_id=?1 UNION ALL SELECT 1 FROM import_tracks WHERE track_id=?1)", [&id], |r| r.get(0))?;
            tx.execute(
                "INSERT OR IGNORE INTO import_tracks VALUES(?1,?2,?3)",
                params![source.to_string(), id, now()],
            )?;
            if exists {
                duplicates += 1;
            } else {
                saved += 1;
            }
        }
        tx.commit()?;
        Ok((saved, duplicates))
    }
    pub fn save_import_progress(&self, progress: &ImportProgress) -> Result<()> {
        let Some(source) = progress.source_id else {
            return Ok(());
        };
        let terminal = !progress.status.active();
        self.connection()?.execute("UPDATE import_sources SET progress_json=?3,active_job=CASE WHEN ?4 THEN NULL ELSE active_job END,refreshed_at=CASE WHEN ?5 THEN ?6 ELSE refreshed_at END WHERE id=?1 AND active_job=?2", params![source.to_string(), progress.job_id.to_string(), serde_json::to_string(progress)?, terminal, matches!(progress.status, ImportStatus::Complete | ImportStatus::Partial) && progress.pages > 0, now()])?;
        Ok(())
    }
    pub fn recover_imports(&self) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        let rows = {
            let mut stmt = tx.prepare(
                "SELECT id,progress_json FROM import_sources WHERE active_job IS NOT NULL",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows
        };
        for (id, json) in rows {
            let json = json.map(|j| -> Result<_> {
                let mut p: ImportProgress = serde_json::from_str(&j)?;
                p.status = ImportStatus::Interrupted;
                p.retry_at = None;
                p.message = Some("Reson closed before import finished. Saved tracks were kept; refresh to continue.".into());
                Ok(serde_json::to_string(&p)?)
            }).transpose()?;
            tx.execute(
                "UPDATE import_sources SET active_job=NULL,progress_json=?2 WHERE id=?1",
                params![id, json],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn import_sources(&self) -> Result<Vec<ImportSource>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT s.id,s.provider,s.provider_user_id,s.url,s.name,s.artwork,s.created_at,s.refreshed_at,(SELECT COUNT(*) FROM import_tracks t WHERE t.source_id=s.id),s.progress_json FROM import_sources s ORDER BY s.created_at DESC,s.id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                ImportProfile {
                    provider: r.get(1)?,
                    provider_user_id: r.get(2)?,
                    url: r.get(3)?,
                    name: r.get(4)?,
                    artwork: r.get(5)?,
                },
                r.get::<_, i64>(6)?,
                r.get::<_, Option<i64>>(7)?,
                r.get::<_, usize>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        rows.map(|row| {
            let (id, profile, created_at, refreshed_at, track_count, progress) = row?;
            Ok(ImportSource {
                id: Uuid::parse_str(&id).map_err(|_| Error::Malformed)?,
                profile,
                created_at,
                refreshed_at,
                track_count,
                progress: progress.map(|j| serde_json::from_str(&j)).transpose()?,
            })
        })
        .collect()
    }
    pub fn import_source(&self, id: Uuid) -> Result<ImportSource> {
        self.import_sources()?
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| Error::Invalid("Imported source no longer exists".into()))
    }
    pub fn import_track_sources(&self) -> Result<HashMap<Uuid, Vec<Uuid>>> {
        let conn = self.connection()?;
        let mut stmt =
            conn.prepare("SELECT track_id,source_id FROM import_tracks ORDER BY added_at DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for row in rows {
            let (track, source) = row?;
            map.entry(Uuid::parse_str(&track).map_err(|_| Error::Malformed)?)
                .or_default()
                .push(Uuid::parse_str(&source).map_err(|_| Error::Malformed)?);
        }
        Ok(map)
    }
    pub fn local_favorite_ids(&self) -> Result<Vec<Uuid>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare("SELECT track_id FROM favorites")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|id| Uuid::parse_str(&id?).map_err(|_| Error::Malformed))
            .collect();
        rows
    }
    pub fn remove_saved_track(&self, id: Uuid) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        tx.execute("INSERT OR IGNORE INTO import_dismissals SELECT source_id,track_id FROM import_tracks WHERE track_id=?1", [id.to_string()])?;
        tx.execute(
            "DELETE FROM import_tracks WHERE track_id=?1",
            [id.to_string()],
        )?;
        tx.execute("DELETE FROM favorites WHERE track_id=?1", [id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
    pub fn remove_import_source(&self, id: Uuid, remove_tracks: bool) -> Result<()> {
        let mut conn = self.connection()?;
        let tx = conn.transaction()?;
        if !remove_tracks {
            tx.execute("INSERT OR IGNORE INTO favorites SELECT track_id,added_at FROM import_tracks WHERE source_id=?1", [id.to_string()])?;
        }
        tx.execute("DELETE FROM import_sources WHERE id=?1", [id.to_string()])?;
        tx.commit()?;
        Ok(())
    }
}
