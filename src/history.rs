//! Bounded resume records; media identity includes path, length and modification time.
use std::{collections::BTreeMap, path::Path, time::UNIX_EPOCH};
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub seconds: f64,
    pub duration: f64,
    pub title: i32,
}
#[derive(Default)]
pub struct History {
    entries: BTreeMap<String, (Position, u64)>,
    tracks: BTreeMap<String, (crate::playback::Tracks, u64)>,
}
impl History {
    pub fn identity(path: &Path) -> Option<String> {
        let full = path.canonicalize().ok()?;
        let meta = full.metadata().ok()?;
        let modified = meta
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_nanos();
        Some(format!(
            "{}|{}|{modified}",
            full.to_str()?.to_lowercase(),
            meta.len()
        ))
    }
    pub fn read(path: &Path) -> Self {
        if path.metadata().is_ok_and(|m| m.len() > 1_048_576) {
            return Self::default();
        }
        let Ok(bytes) = std::fs::read(path) else {
            return Self::default();
        };
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return Self::default();
        };
        let mut history = Self::default();
        if value["format"] != 1 {
            return history;
        }
        if let Some(entries) = value["entries"].as_array() {
            for row in entries.iter().take(500) {
                if let (Some(key), Some(seconds), Some(duration), Some(title), Some(updated)) = (
                    row["key"].as_str(),
                    row["seconds"].as_f64(),
                    row["duration"].as_f64(),
                    row["title"].as_i64(),
                    row["updated"].as_u64(),
                ) && key.len() <= 8192
                    && (-1..=99999).contains(&title)
                {
                    history.remember(
                        key.to_owned(),
                        Position {
                            seconds,
                            duration,
                            title: title as i32,
                        },
                        updated,
                    );
                }
            }
        }
        if let Some(entries) = value["tracks"].as_array() {
            for row in entries.iter().take(500) {
                if let (Some(key), Some(tracks), Some(updated)) = (
                    row["key"].as_str(),
                    crate::playback::Tracks::decode(&row["choice"]),
                    row["updated"].as_u64(),
                ) && key.len() <= 8192
                {
                    history.remember_tracks(key.into(), tracks, updated);
                }
            }
        }
        history
    }
    pub fn get(&self, key: &str) -> Option<Position> {
        self.entries.get(key).map(|e| e.0.clone())
    }
    pub fn get_tracks(&self, key: &str) -> Option<crate::playback::Tracks> {
        self.tracks.get(key).map(|r| r.0.clone())
    }
    pub fn remember_tracks(&mut self, key: String, tracks: crate::playback::Tracks, updated: u64) {
        if self.tracks.get(&key).is_some_and(|(_, old)| *old > updated) {
            return;
        }
        self.tracks.insert(key, (tracks, updated));
        while self.tracks.len() > 500
            || self
                .tracks
                .iter()
                .map(|(k, (v, _))| {
                    serde_json::to_string(k).unwrap().len() + v.encode().to_string().len() + 100
                })
                .sum::<usize>()
                > 400_000
        {
            let Some(key) = self
                .tracks
                .iter()
                .min_by_key(|(_, v)| v.1)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.tracks.remove(&key);
        }
    }
    pub fn remember(&mut self, key: String, position: Position, updated: u64) {
        if !position.seconds.is_finite()
            || !position.duration.is_finite()
            || position.duration <= 0.0
        {
            return;
        }
        if position.seconds < 10.0 || position.seconds >= position.duration - 10.0 {
            self.entries.remove(&key);
            return;
        }
        self.entries.insert(key, (position, updated));
        while self.entries.len() > 500
            || self
                .entries
                .keys()
                .map(|k| serde_json::to_string(k).unwrap().len() + 180)
                .sum::<usize>()
                > 400_000
        {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, v)| v.1)
                .map(|(k, _)| k.clone())
            else {
                break;
            };
            self.entries.remove(&key);
        }
    }
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"format":1,"entries":self.entries.iter().map(|(key,(p,updated))| serde_json::json!({"key":key,"seconds":p.seconds,"duration":p.duration,"title":p.title,"updated":updated})).collect::<Vec<_>>(),"tracks":self.tracks.iter().map(|(key,(choice,updated))|serde_json::json!({"key":key,"choice":choice.encode(),"updated":updated})).collect::<Vec<_>>()})).expect("validated finite resume positions")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selections_survive_completed_position_and_stale_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("resume.json");
        let t = crate::playback::Tracks::capture(
            17,
            &serde_json::json!([{"type":"audio","id":2,"lang":"spa"}]),
            "2",
            "no",
        )
        .unwrap();
        let mut h = History::default();
        h.remember_tracks("movie".into(), t.clone(), 20);
        let mut stale = t.clone();
        stale.title = 1;
        h.remember_tracks("movie".into(), stale, 10);
        h.remember(
            "movie".into(),
            Position {
                seconds: 119.0,
                duration: 120.0,
                title: 17,
            },
            30,
        );
        std::fs::write(&path, h.encode()).unwrap();
        let h = History::read(&path);
        assert!(h.get("movie").is_none());
        assert_eq!(h.get_tracks("movie"), Some(t));
    }
    #[test]
    fn history_evicts_oldest_and_respects_its_read_size_limit() {
        let mut h = History::default();
        let p = Position {
            seconds: 30.0,
            duration: 120.0,
            title: -1,
        };
        for i in 0..600 {
            h.remember(i.to_string(), p.clone(), i);
        }
        assert_eq!(h.entries.len(), 500);
        assert!(h.get("0").is_none());
        assert!(h.get("599").is_some());
        for i in 600..800 {
            h.remember(format!("{i}{}", "x".repeat(8000)), p.clone(), i);
        }
        assert!(h.encode().len() < 1_048_576);
    }
    #[test]
    fn resume_roundtrip_completion_invalid_and_replaced_media() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("movie.mkv");
        std::fs::write(&media, b"original").unwrap();
        let key = History::identity(&media).unwrap();
        let mut h = History::default();
        let p = Position {
            seconds: 42.0,
            duration: 120.0,
            title: 17,
        };
        h.remember(key.clone(), p.clone(), 1);
        h.remember(
            "bad".into(),
            Position {
                seconds: f64::NAN,
                ..p.clone()
            },
            2,
        );
        assert!(h.get("bad").is_none());
        let file = dir.path().join("history.json");
        std::fs::write(&file, h.encode()).unwrap();
        assert_eq!(History::read(&file).get(&key), Some(p.clone()));
        std::fs::write(&media, b"different movie").unwrap();
        assert_ne!(History::identity(&media).unwrap(), key);
        h.remember(
            key.clone(),
            Position {
                seconds: 115.0,
                ..p
            },
            3,
        );
        assert!(h.get(&key).is_none());
        std::fs::write(&file, b"broken JSON").unwrap();
        assert!(History::read(&file).entries.is_empty());
    }
}
