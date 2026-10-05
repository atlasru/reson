use crate::{
    error::{Error, Result},
    models::Track,
};
use rand::{seq::SliceRandom, SeedableRng};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    Queue,
    Track,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub entry_id: Uuid,
    pub track: Track,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Queue {
    pub entries: Vec<QueueEntry>,
    pub current: Option<Uuid>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub order: Vec<Uuid>,
}
impl Queue {
    pub const LIMIT: usize = 5000;
    pub fn current(&self) -> Option<&QueueEntry> {
        self.current
            .and_then(|id| self.entries.iter().find(|e| e.entry_id == id))
    }
    pub fn enqueue(&mut self, tracks: Vec<Track>, next: bool) -> Result<Vec<Uuid>> {
        if self.entries.len() + tracks.len() > Self::LIMIT {
            return Err(Error::Invalid("Queue limit is 5000 tracks".into()));
        }
        let entries = tracks
            .into_iter()
            .map(|track| QueueEntry {
                entry_id: Uuid::new_v4(),
                track,
            })
            .collect::<Vec<_>>();
        let ids = entries.iter().map(|e| e.entry_id).collect::<Vec<_>>();
        let index = if next {
            self.current
                .and_then(|id| self.entries.iter().position(|e| e.entry_id == id))
                .map(|i| i + 1)
                .unwrap_or(self.entries.len())
        } else {
            self.entries.len()
        };
        self.entries.splice(index..index, entries);
        if self.shuffle {
            let order_index = if next {
                self.current
                    .and_then(|id| self.order.iter().position(|x| *x == id))
                    .map(|i| i + 1)
                    .unwrap_or(self.order.len())
            } else {
                self.order.len()
            };
            self.order.splice(order_index..order_index, ids.clone());
        } else {
            self.order = self.entries.iter().map(|e| e.entry_id).collect();
        }
        Ok(ids)
    }
    pub fn replace(&mut self, tracks: Vec<Track>, index: usize) -> Result<()> {
        if tracks.is_empty() || index >= tracks.len() || tracks.len() > Self::LIMIT {
            return Err(Error::Invalid("Choose an existing track".into()));
        }
        self.entries = tracks
            .into_iter()
            .map(|track| QueueEntry {
                entry_id: Uuid::new_v4(),
                track,
            })
            .collect();
        self.current = Some(self.entries[index].entry_id);
        self.rebuild_order(None);
        Ok(())
    }
    pub fn select(&mut self, id: Uuid) -> Result<()> {
        if !self.entries.iter().any(|e| e.entry_id == id) {
            return Err(Error::Invalid("Queue entry no longer exists".into()));
        }
        self.current = Some(id);
        Ok(())
    }
    pub fn advance(&mut self, automatic: bool) -> Option<Uuid> {
        if automatic && self.repeat == RepeatMode::Track && self.current.is_some() {
            return self.current;
        }
        let next = match self
            .current
            .and_then(|id| self.order.iter().position(|x| *x == id))
        {
            Some(i) if i + 1 < self.order.len() => Some(self.order[i + 1]),
            Some(_) if self.repeat == RepeatMode::Queue => self.order.first().copied(),
            None => self.order.first().copied(),
            _ => None,
        };
        if next.is_some() {
            self.current = next;
        }
        next
    }
    pub fn previous(&mut self) -> Option<Uuid> {
        let index = self
            .current
            .and_then(|id| self.order.iter().position(|x| *x == id))?;
        let prev = if index > 0 {
            Some(self.order[index - 1])
        } else if self.repeat == RepeatMode::Queue {
            self.order.last().copied()
        } else {
            self.current
        };
        self.current = prev;
        prev
    }
    pub fn remove(&mut self, id: Uuid) -> Result<bool> {
        let index = self
            .entries
            .iter()
            .position(|e| e.entry_id == id)
            .ok_or_else(|| Error::Invalid("Queue entry no longer exists".into()))?;
        let was_current = self.current == Some(id);
        let next = self
            .order
            .iter()
            .position(|x| *x == id)
            .and_then(|i| self.order.get(i + 1))
            .copied();
        self.entries.remove(index);
        self.order.retain(|x| *x != id);
        if was_current {
            self.current = next.or_else(|| self.order.last().copied());
        }
        Ok(was_current)
    }
    pub fn reorder(&mut self, id: Uuid, to: usize) -> Result<()> {
        if to >= self.entries.len() {
            return Err(Error::Invalid("Invalid queue position".into()));
        }
        let from = self
            .entries
            .iter()
            .position(|e| e.entry_id == id)
            .ok_or_else(|| Error::Invalid("Queue entry no longer exists".into()))?;
        let entry = self.entries.remove(from);
        self.entries.insert(to, entry);
        if !self.shuffle {
            self.order = self.entries.iter().map(|e| e.entry_id).collect();
        }
        Ok(())
    }
    pub fn set_shuffle(&mut self, enabled: bool) {
        self.shuffle = enabled;
        self.rebuild_order(None);
    }
    fn rebuild_order(&mut self, seed: Option<u64>) {
        self.order = self.entries.iter().map(|e| e.entry_id).collect();
        if self.shuffle {
            if let Some(seed) = seed {
                self.order
                    .shuffle(&mut rand::rngs::StdRng::seed_from_u64(seed));
            } else {
                self.order.shuffle(&mut rand::rng());
            }
            if let Some(current) = self.current {
                self.order.retain(|id| *id != current);
                self.order.insert(0, current);
            }
        }
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.current = None;
    }
    pub fn validate(&mut self) {
        self.entries.truncate(Self::LIMIT);
        let mut seen = std::collections::HashSet::new();
        self.entries.retain(|e| seen.insert(e.entry_id));
        let ids = self
            .entries
            .iter()
            .map(|e| e.entry_id)
            .collect::<std::collections::HashSet<_>>();
        let mut seen = std::collections::HashSet::new();
        self.order.retain(|id| ids.contains(id) && seen.insert(*id));
        self.order.extend(
            self.entries
                .iter()
                .filter(|e| !seen.contains(&e.entry_id))
                .map(|e| e.entry_id),
        );
        if self.current.is_some_and(|id| !ids.contains(&id)) {
            self.current = None;
        }
        if !self.shuffle {
            self.order = self.entries.iter().map(|e| e.entry_id).collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tracks(n: usize) -> Vec<Track> {
        (0..n)
            .map(|i| Track {
                internal_id: Uuid::new_v4(),
                title: i.to_string(),
                artists: vec![],
                album: None,
                artwork: None,
                duration_ms: 10000,
                explicit: None,
                availability: Default::default(),
                sources: vec![],
            })
            .collect()
    }
    #[test]
    fn ordering_and_insert_next() {
        let mut q = Queue::default();
        q.replace(tracks(3), 0).unwrap();
        let old = q.entries[1].entry_id;
        let id = q.enqueue(tracks(1), true).unwrap()[0];
        assert_eq!(q.advance(false), Some(id));
        assert_eq!(q.advance(false), Some(old));
        assert_eq!(q.previous(), Some(id));
    }
    #[test]
    fn repeat_and_manual_next() {
        let mut q = Queue::default();
        q.replace(tracks(2), 0).unwrap();
        let first = q.current;
        q.repeat = RepeatMode::Track;
        assert_eq!(q.advance(true), first);
        assert_ne!(q.advance(false), first);
        q.repeat = RepeatMode::Off;
        assert_eq!(q.advance(true), None);
        q.repeat = RepeatMode::Queue;
        assert_eq!(q.advance(true), first);
    }
    #[test]
    fn shuffle_preserves_identity_and_membership() {
        let mut q = Queue::default();
        q.replace(tracks(20), 8).unwrap();
        let current = q.current;
        q.shuffle = true;
        q.rebuild_order(Some(42));
        assert_eq!(q.order[0], current.unwrap());
        assert_eq!(
            q.order
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            20
        );
        q.set_shuffle(false);
        assert_eq!(q.current, current);
        assert_eq!(
            q.order,
            q.entries.iter().map(|e| e.entry_id).collect::<Vec<_>>()
        );
    }
    #[test]
    fn removal_reorder_and_corrupt_restore() {
        let mut q = Queue::default();
        q.replace(tracks(3), 0).unwrap();
        let end = q.entries[2].entry_id;
        q.reorder(end, 0).unwrap();
        assert_eq!(q.entries[0].entry_id, end);
        let current = q.current.unwrap();
        assert!(q.remove(current).unwrap());
        assert_eq!(q.current.unwrap(), q.entries[1].entry_id);
    }
}
