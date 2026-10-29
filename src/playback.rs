//! Stable per-media track choices, including disabled and external subtitles.
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    Off,
    Track {
        id: u64,
        source: Option<i64>,
        index: Option<i64>,
        language: String,
        name: String,
        codec: String,
        external: Option<String>,
    },
}
fn text(v: &Value, key: &str, max: usize) -> String {
    v[key]
        .as_str()
        .filter(|s| s.len() <= max)
        .unwrap_or_default()
        .into()
}
pub fn local_path(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32768 && !s.contains("://") && !s.contains('\0')
}
fn path_key(s: &str) -> String {
    s.replace('\\', "/").to_lowercase()
}
impl Selection {
    fn track(v: &Value) -> Option<Self> {
        let id = v["id"].as_u64().filter(|id| (1..=1_000_000).contains(id))?;
        let external = v["external-filename"]
            .as_str()
            .filter(|s| local_path(s) && v["type"] != "audio")
            .map(str::to_owned);
        Some(Self::Track {
            id,
            source: v["src-id"].as_i64(),
            index: v["ff-index"].as_i64(),
            language: text(v, "lang", 128),
            name: text(v, "title", 1024),
            codec: text(v, "codec", 128),
            external,
        })
    }
    fn decode(v: &Value) -> Option<Self> {
        if v.as_str() == Some("no") {
            Some(Self::Off)
        } else {
            Self::track(v)
        }
    }
    fn encode(&self) -> Value {
        match self {
            Self::Off => json!("no"),
            Self::Track {
                id,
                source,
                index,
                language,
                name,
                codec,
                external,
            } => {
                json!({"id":id,"src-id":source,"ff-index":index,"lang":language,"title":name,"codec":codec,"external-filename":external})
            }
        }
    }
    fn capture(list: &[Value], kind: &str, id: &str) -> Option<Self> {
        if id == "no" {
            return Some(Self::Off);
        }
        let id = id.parse::<u64>().ok()?;
        list.iter()
            .find(|r| r["type"] == kind && r["id"].as_u64() == Some(id))
            .and_then(Self::track)
    }
    pub fn resolve(&self, list: &[Value], kind: &str) -> Option<String> {
        let Self::Track {
            id,
            source,
            index,
            language,
            name,
            codec,
            external,
        } = self
        else {
            return Some("no".into());
        };
        let mut ranked = Vec::new();
        for r in list.iter().filter(|r| r["type"] == kind) {
            let Some(Self::Track {
                id: other,
                source: os,
                index: oi,
                language: ol,
                name: on,
                codec: oc,
                external: oe,
            }) = Self::track(r)
            else {
                continue;
            };
            if let Some(p) = external {
                if oe.as_ref().is_some_and(|q| path_key(p) == path_key(q)) {
                    ranked.push((20_000, other));
                }
                continue;
            }
            if oe.is_some() {
                continue;
            }
            let stable = source.is_some() && source == &os;
            if !stable
                && ((!language.is_empty() && !ol.is_empty() && !language.eq_ignore_ascii_case(&ol))
                    || (!name.is_empty() && !on.is_empty() && name != &on)
                    || (!codec.is_empty() && !oc.is_empty() && codec != &oc))
            {
                continue;
            }
            let score = i32::from(stable) * 10_000
                + i32::from(index.is_some() && index == &oi) * 5_000
                + i32::from(!language.is_empty() && language.eq_ignore_ascii_case(&ol)) * 500
                + i32::from(!name.is_empty() && name == &on) * 1_000
                + i32::from(!codec.is_empty() && codec == &oc) * 100
                + i32::from(id == &other) * 10;
            if score > 0 {
                ranked.push((score, other));
            }
        }
        ranked.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        ranked.first().map(|(_, id)| id.to_string())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Tracks {
    pub title: i32,
    pub audio: Option<Selection>,
    pub subtitle: Option<Selection>,
    pub external_subs: Vec<String>,
}
impl Tracks {
    pub fn capture(title: i32, list: &Value, audio: &str, subtitle: &str) -> Option<Self> {
        let list = list.as_array()?;
        if list.is_empty() || list.len() > 256 {
            return None;
        }
        let audio = Selection::capture(list, "audio", audio);
        let subtitle = Selection::capture(list, "sub", subtitle);
        if audio.is_none() && list.iter().any(|r| r["type"] == "audio") {
            return None;
        }
        let external_subs = list
            .iter()
            .filter(|r| r["type"] == "sub" && r["external"] == true)
            .filter_map(|r| r["external-filename"].as_str())
            .filter(|s| local_path(s))
            .take(32)
            .map(str::to_owned)
            .collect();
        Some(Self {
            title,
            audio,
            subtitle,
            external_subs,
        })
    }
    pub fn encode(&self) -> Value {
        json!({"title":self.title,"audio":self.audio.as_ref().map(Selection::encode),"subtitle":self.subtitle.as_ref().map(Selection::encode),"external_subs":self.external_subs})
    }
    pub fn decode(v: &Value) -> Option<Self> {
        let title = v["title"].as_i64().filter(|n| (-1..=99999).contains(n))? as i32;
        let external_subs = v["external_subs"]
            .as_array()?
            .iter()
            .take(32)
            .filter_map(Value::as_str)
            .filter(|s| local_path(s))
            .map(str::to_owned)
            .collect();
        Some(Self {
            title,
            audio: Selection::decode(&v["audio"]),
            subtitle: Selection::decode(&v["subtitle"]),
            external_subs,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn choices_survive_reordered_ids_and_disabled_subtitles() {
        let old = json!([{"type":"audio","id":1,"src-id":4352,"lang":"eng","codec":"dts"},{"type":"audio","id":2,"src-id":4353,"lang":"spa","codec":"dts"}]);
        let t = Tracks::capture(17, &old, "2", "no").unwrap();
        let new = json!([{"type":"audio","id":2,"src-id":4352,"lang":"eng","codec":"dts"},{"type":"audio","id":4,"src-id":4353,"lang":"spa","codec":"dts"}]);
        assert_eq!(
            t.audio
                .as_ref()
                .unwrap()
                .resolve(new.as_array().unwrap(), "audio")
                .as_deref(),
            Some("4")
        );
        assert_eq!(
            t.subtitle
                .as_ref()
                .unwrap()
                .resolve(new.as_array().unwrap(), "sub")
                .as_deref(),
            Some("no")
        );
        assert_eq!(Tracks::decode(&t.encode()), Some(t));
    }
    #[test]
    fn external_selection_is_not_confused_with_an_embedded_track() {
        let old = json!([{"type":"sub","id":2,"external":true,"external-filename":"C:\\Subtitles\\Spanish.srt"}]);
        let t = Tracks::capture(-1, &old, "no", "2").unwrap();
        let new = json!([{"type":"sub","id":2},{"type":"sub","id":7,"external":true,"external-filename":"c:/subtitles/spanish.srt"}]);
        assert_eq!(
            t.subtitle
                .unwrap()
                .resolve(new.as_array().unwrap(), "sub")
                .as_deref(),
            Some("7")
        );
        assert!(!local_path("https://example.org/a.srt"));
    }
}
