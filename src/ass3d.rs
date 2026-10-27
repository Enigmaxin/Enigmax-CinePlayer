//! Conservative detection of eye pairs in native libass event serialization.
//! This module never edits dialogue, timing, styles or geometry.
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packing {
    Sbs,
    Ou,
}
impl Packing {
    pub fn label(self) -> &'static str {
        match self {
            Self::Sbs => "SBS",
            Self::Ou => "OU",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Auto,
    Off,
    Sbs,
    Ou,
}
impl Mode {
    pub const ALL: [Self; 4] = [Self::Auto, Self::Off, Self::Sbs, Self::Ou];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automático · detectar eventos duplicados",
            Self::Off => "Subtítulos 2D · no transformar",
            Self::Sbs => "Forzar ASS 3D SBS",
            Self::Ou => "Forzar ASS 3D OU",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Canvas {
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, PartialEq)]
struct Style {
    alignment: u8,
    margin_l: f64,
    margin_r: f64,
    margin_v: f64,
    signature: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    pub canvas: Canvas,
    styles: HashMap<String, Style>,
}
#[derive(Clone, Copy, Debug)]
struct Point {
    x: f64,
    y: f64,
}
#[derive(Clone, Debug)]
struct Event {
    identity: String,
    points: Vec<Point>,
}
fn finite(s: &str) -> Option<f64> {
    let n = s.trim().parse::<f64>().ok()?;
    n.is_finite().then_some(n)
}
fn field<'a>(format: &[String], values: &[&'a str], name: &str) -> Option<&'a str> {
    format
        .iter()
        .position(|v| v == name)
        .and_then(|i| values.get(i).copied())
        .map(str::trim)
}
impl Header {
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() > 1024 * 1024 {
            return None;
        }
        let (mut x, mut y) = (None, None);
        let mut section = String::new();
        let mut format = Vec::new();
        let mut styles = HashMap::new();
        for line in text.lines() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.starts_with('[') {
                section = line.to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            if section == "[script info]" {
                match key.as_str() {
                    "playresx" => x = finite(value),
                    "playresy" => y = finite(value),
                    _ => {}
                }
            }
            if section == "[v4+ styles]" || section == "[v4 styles]" {
                if key == "format" {
                    format = value
                        .split(',')
                        .map(|s| s.trim().to_ascii_lowercase())
                        .collect();
                }
                if key == "style" && styles.len() < 512 {
                    let values = value.split(',').collect::<Vec<_>>();
                    let Some(name) = field(&format, &values, "name") else {
                        continue;
                    };
                    let mut alignment = field(&format, &values, "alignment")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(2);
                    if section == "[v4 styles]" {
                        alignment = match alignment {
                            5 => 7,
                            6 => 8,
                            7 => 9,
                            9 => 4,
                            10 => 5,
                            11 => 6,
                            v => v,
                        };
                    }
                    let signature = format
                        .iter()
                        .zip(&values)
                        .filter(|(k, _)| {
                            !["name", "alignment", "marginl", "marginr", "marginv"]
                                .contains(&k.as_str())
                        })
                        .map(|(k, v)| format!("{k}={};", v.trim()))
                        .collect::<String>();
                    styles.insert(
                        name.into(),
                        Style {
                            alignment,
                            margin_l: field(&format, &values, "marginl")
                                .and_then(finite)
                                .unwrap_or(0.0),
                            margin_r: field(&format, &values, "marginr")
                                .and_then(finite)
                                .unwrap_or(0.0),
                            margin_v: field(&format, &values, "marginv")
                                .and_then(finite)
                                .unwrap_or(0.0),
                            signature,
                        },
                    );
                }
            }
        }
        let (width, height) = (x?, y?);
        if !(2.0..=32768.0).contains(&width) || !(2.0..=32768.0).contains(&height) {
            return None;
        }
        Some(Self {
            canvas: Canvas { width, height },
            styles,
        })
    }
    /// Return a layout only when simultaneous copies match in content, style,
    /// timing and layer, with corresponding positions in opposing halves.
    pub fn detect(&self, events: &str) -> Option<Packing> {
        if events.len() > 256 * 1024 {
            return None;
        }
        let events = events
            .lines()
            .take(128)
            .filter_map(|line| self.event(line))
            .collect::<Vec<_>>();
        let mut result = None;
        for (i, a) in events.iter().enumerate() {
            for b in events.iter().skip(i + 1) {
                if a.identity != b.identity || a.points.len() != b.points.len() {
                    continue;
                }
                for packing in [Packing::Sbs, Packing::Ou] {
                    if a.points
                        .iter()
                        .zip(&b.points)
                        .all(|(a, b)| self.pair(*a, *b, packing))
                    {
                        if result.is_some_and(|old| old != packing) {
                            return None;
                        }
                        result = Some(packing);
                    }
                }
            }
        }
        result
    }
    fn pair(&self, a: Point, b: Point, packing: Packing) -> bool {
        let c = self.canvas;
        if [a.x, b.x]
            .into_iter()
            .any(|v| !(0.0..=c.width).contains(&v))
            || [a.y, b.y]
                .into_iter()
                .any(|v| !(0.0..=c.height).contains(&v))
        {
            return false;
        }
        let (u1, v1, u2, v2) = match packing {
            Packing::Sbs => (a.x / c.width, a.y / c.height, b.x / c.width, b.y / c.height),
            Packing::Ou => (a.y / c.height, a.x / c.width, b.y / c.height, b.x / c.width),
        };
        let (lo, hi) = if u1 < u2 { (u1, u2) } else { (u2, u1) };
        lo <= 0.47 && hi >= 0.53 && (hi - lo - 0.5).abs() <= 0.04 && (v1 - v2).abs() <= 0.01
    }
    fn event(&self, line: &str) -> Option<Event> {
        let line = line.trim().strip_prefix("Dialogue:")?.trim();
        let f = line.splitn(10, ',').collect::<Vec<_>>();
        if f.len() != 10 {
            return None;
        }
        let style = self.styles.get(f[3].trim())?;
        let text = f[9];
        let mut visible = String::new();
        let mut signature = String::new();
        let mut points = Vec::new();
        let mut alignment = style.alignment;
        let mut tail = text;
        while !tail.is_empty() {
            if let Some(body) = tail.strip_prefix('{') {
                let (tags, rest) = body.split_once('}')?;
                tail = rest;
                // Nested transforms can animate positions. Leave these to the
                // manual override rather than treating an inner tag as an anchor.
                if tags.contains("\\t(") {
                    return None;
                }
                let mut tags = tags;
                while let Some(pos) = tags.find('\\') {
                    signature.push_str(&tags[..pos]);
                    tags = &tags[pos + 1..];
                    if tags.starts_with("pos(") || tags.starts_with("move(") {
                        let moving = tags.starts_with("move(");
                        let start = if moving { 5 } else { 4 };
                        let end = tags[start..].find(')')? + start;
                        let args = tags[start..end]
                            .split(',')
                            .map(finite)
                            .collect::<Option<Vec<_>>>()?;
                        if (!moving && args.len() != 2)
                            || (moving && args.len() != 4 && args.len() != 6)
                            || !points.is_empty()
                        {
                            return None;
                        }
                        points.push(Point {
                            x: args[0],
                            y: args[1],
                        });
                        if moving {
                            points.push(Point {
                                x: args[2],
                                y: args[3],
                            });
                        }
                        signature.push_str(if moving { "\\move" } else { "\\pos" });
                        if args.len() == 6 {
                            signature.push_str(&format!("{}/{}", args[4], args[5]));
                        }
                        tags = &tags[end + 1..];
                    } else {
                        let end = tags.find('\\').unwrap_or(tags.len());
                        let tag = &tags[..end];
                        if let Some(n) = tag.strip_prefix("an").and_then(|n| n.parse::<u8>().ok()) {
                            alignment = n;
                        }
                        signature.push('\\');
                        signature.push_str(tag);
                        tags = &tags[end..];
                    }
                }
                signature.push_str(tags);
            } else {
                let end = tail.find('{').unwrap_or(tail.len());
                visible.push_str(&tail[..end]);
                tail = &tail[end..];
            }
        }
        if visible.trim().is_empty() {
            return None;
        }
        if points.is_empty() {
            let margin =
                |n: usize, default: f64| finite(f[n]).filter(|n| *n != 0.0).unwrap_or(default);
            let l = margin(5, style.margin_l);
            let r = margin(6, style.margin_r);
            let v = margin(7, style.margin_v);
            if !(1..=9).contains(&alignment) {
                return None;
            }
            let x = match (alignment - 1) % 3 {
                0 => l,
                1 => (self.canvas.width + l - r) / 2.0,
                _ => self.canvas.width - r,
            };
            let y = match (alignment - 1) / 3 {
                0 => self.canvas.height - v,
                1 => self.canvas.height / 2.0,
                _ => v,
            };
            points.push(Point { x, y });
        }
        Some(Event {
            identity: format!(
                "{}|{}|{}|{}|{}|{}|{}|{}",
                f[0].trim(),
                f[1].trim(),
                f[2].trim(),
                f[8].trim(),
                style.signature,
                alignment,
                signature,
                visible
            ),
            points,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn header() -> Header {
        Header::parse("[Script Info]\nPlayResX: 3840\nPlayResY: 1080\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, Alignment, MarginL, MarginR, MarginV\nStyle: Default,Arial,54,2,0,0,0").unwrap()
    }
    fn cue(text: &str) -> String {
        format!("Dialogue: 0,0:00:02.73,0:00:05.77,Default,,0,0,0,,{text}")
    }
    #[test]
    fn titanic_canvas_and_disparity() {
        let h = header();
        let t = [
            cue("{\\an2\\pos(968,940)}Texto\\Ndoble"),
            cue("{\\an2\\pos(2872,940)}Texto\\Ndoble"),
        ]
        .join("\n");
        assert_eq!(h.detect(&t), Some(Packing::Sbs));
        assert_eq!(h.canvas.width, 3840.0);
    }
    #[test]
    fn ou_and_moving_pairs() {
        let mut h = header();
        h.canvas = Canvas {
            width: 1920.0,
            height: 2160.0,
        };
        let t = [cue("{\\pos(960,940)}Texto"), cue("{\\pos(960,2020)}Texto")].join("\n");
        assert_eq!(h.detect(&t), Some(Packing::Ou));
        let h = header();
        let t = [
            cue("{\\move(960,900,970,920,0,1000)}Texto"),
            cue("{\\move(2880,900,2890,920,0,1000)}Texto"),
        ]
        .join("\n");
        assert_eq!(h.detect(&t), Some(Packing::Sbs));
    }
    #[test]
    fn ordinary_ass_and_conflicting_content_are_not_stereo() {
        let h = header();
        for t in [
            [cue("{\\pos(960,940)}Texto"), cue("{\\pos(2880,940)}Otro")],
            [cue("{\\pos(960,940)}Texto"), cue("{\\pos(960,940)}Texto")],
            [cue("Texto"), cue("Texto")],
        ] {
            assert_eq!(h.detect(&t.join("\n")), None);
        }
        assert_eq!(
            h.detect(&[cue("{\\pos(NaN,940)}Texto"), cue("{\\pos(2880,940)}Texto")].join("\n")),
            None
        );
    }
    #[test]
    fn header_requires_bounded_canvas() {
        for s in [
            "[Script Info]\nPlayResX: 3840",
            "[Script Info]\nPlayResX: NaN\nPlayResY: 1080",
            "[Script Info]\nPlayResX: 999999\nPlayResY: 1080",
        ] {
            assert!(Header::parse(s).is_none());
        }
    }
    #[test]
    fn mirrored_style_margins_are_recognized() {
        let h=Header::parse("[Script Info]\nPlayResX: 3840\nPlayResY: 1080\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, Alignment, MarginL, MarginR, MarginV\nStyle: Left,Arial,54,2,0,1920,100\nStyle: Right,Arial,54,2,1920,0,100").unwrap();
        assert_eq!(
            h.detect(
                &[
                    cue("Texto").replace(",Default,", ",Left,"),
                    cue("Texto").replace(",Default,", ",Right,")
                ]
                .join("\n")
            ),
            Some(Packing::Sbs)
        );
    }
    #[test]
    fn different_timing_and_nested_motion_do_not_trigger_detection() {
        let h = header();
        let a = cue("{\\pos(968,940)}Texto");
        let b = cue("{\\pos(2872,940)}Texto").replace("0:00:02.73", "0:00:03.00");
        assert_eq!(h.detect(&[a, b].join("\n")), None);
        assert_eq!(
            h.detect(
                &[
                    cue("{\\t(0,1000,\\pos(968,940))}Texto"),
                    cue("{\\t(0,1000,\\pos(2872,940))}Texto")
                ]
                .join("\n")
            ),
            None
        );
    }
}
