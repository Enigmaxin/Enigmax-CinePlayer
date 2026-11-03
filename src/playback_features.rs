//! Optional playback policy. No extra decoder, image analysis or per-frame polling.
use crate::{mpv::Mpv, player::State, windows};
use cineplayer_core::stereo::{Quality, Settings};
use serde_json::{Value, json};
use std::{path::Path, time::Instant};

#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub adaptive: bool,
    pub refresh: bool,
    pub cursor: bool,
    pub track_rule: u8,
    pub audio_mode: u8,
    pub deinterlace: bool,
    pub sub_scale: f64,
    pub sub_color: String,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            adaptive: true,
            refresh: false,
            cursor: true,
            track_rule: 0,
            audio_mode: 0,
            deinterlace: false,
            sub_scale: 1.0,
            sub_color: "#FFFFFF".into(),
        }
    }
}
impl Preferences {
    fn decode(v: &Value) -> Self {
        let mut p = Self::default();
        if v["version"] != 1 {
            return p;
        }
        for (key, dest) in [
            ("adaptive", &mut p.adaptive),
            ("refresh", &mut p.refresh),
            ("cursor", &mut p.cursor),
            ("deinterlace", &mut p.deinterlace),
        ] {
            if let Some(b) = v[key].as_bool() {
                *dest = b;
            }
        }
        p.track_rule = v["track_rule"].as_u64().filter(|n| *n <= 2).unwrap_or(0) as u8;
        p.audio_mode = v["audio_mode"].as_u64().filter(|n| *n <= 2).unwrap_or(0) as u8;
        p.sub_scale = v["sub_scale"]
            .as_f64()
            .filter(|n| n.is_finite() && (0.5..=2.5).contains(n))
            .unwrap_or(1.0);
        if let Some(c) = v["sub_color"].as_str().filter(|c| {
            c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())
        }) {
            p.sub_color = c.to_owned();
        }
        p
    }
    fn encode(&self) -> Value {
        json!({"version":1,"adaptive":self.adaptive,"refresh":self.refresh,"cursor":self.cursor,
            "track_rule":self.track_rule,"audio_mode":self.audio_mode,"deinterlace":self.deinterlace,
            "sub_scale":self.sub_scale,"sub_color":self.sub_color})
    }
}

#[derive(Clone, Debug, PartialEq)]
struct VisualPreferences {
    sub_pos: i32,
    left_gain: f32,
    right_gain: f32,
    crosstalk: f32,
}
impl VisualPreferences {
    fn from_settings(p: &Settings) -> Self {
        Self {
            sub_pos: p.sub_pos,
            left_gain: p.left_gain,
            right_gain: p.right_gain,
            crosstalk: p.crosstalk,
        }
    }
    fn restore(&self, p: &mut Settings) {
        p.sub_pos = self.sub_pos;
        p.left_gain = self.left_gain;
        p.right_gain = self.right_gain;
        p.crosstalk = self.crosstalk;
    }
    fn decode(v: &Value) -> Option<Self> {
        let p = Settings {
            sub_pos: i32::try_from(v["sub_pos"].as_i64()?).ok()?,
            left_gain: v["left_gain"].as_f64()? as f32,
            right_gain: v["right_gain"].as_f64()? as f32,
            crosstalk: v["crosstalk"].as_f64()? as f32,
            ..Settings::default()
        };
        p.validate().ok()?;
        Some(Self::from_settings(&p))
    }
    fn encode(&self) -> Value {
        json!({"sub_pos":self.sub_pos,"left_gain":self.left_gain,"right_gain":self.right_gain,"crosstalk":self.crosstalk})
    }
}

pub struct Features {
    pub prefs: Preferences,
    saved_visual: Option<VisualPreferences>,
    applied: Option<(Preferences, u8)>,
    last_tick: Instant,
    cache_cap: u64,
    cache_seconds: f64,
    cache_low: bool,
    applied_audio: Option<(u8, u8)>,
    pub status: String,
    screen: String,
    screen_defaults: Value,
    screen_override: bool,
    track_key: String,
    pub display: crate::display::Display,
}
impl Default for Features {
    fn default() -> Self {
        Self {
            prefs: Preferences::default(),
            saved_visual: None,
            applied: None,
            last_tick: Instant::now(),
            cache_cap: 0,
            cache_seconds: 2.0,
            cache_low: false,
            applied_audio: None,
            status: String::new(),
            screen: String::new(),
            screen_defaults: Value::Null,
            screen_override: false,
            track_key: String::new(),
            display: Default::default(),
        }
    }
}
fn read(path: &Path) -> Value {
    if !path.metadata().is_ok_and(|m| m.len() <= 256 * 1024) {
        return Value::Null;
    }
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn write(path: &Path, value: &Value) -> Result<(), String> {
    let dir = path.parent().ok_or("Ruta de preferencias inválida")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut f = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    use std::io::Write;
    f.write_all(value.to_string().as_bytes())
        .map_err(|e| e.to_string())?;
    f.as_file().sync_all().map_err(|e| e.to_string())?;
    windows::replace_file(f.path(), path)
}
pub fn cache_budget(bitrate: f64, low: bool, stalled: bool) -> (u64, f64) {
    let seconds = if stalled { 8.0 } else { 3.0 };
    let rate = if bitrate.is_finite() && bitrate > 0.0 {
        bitrate
    } else {
        32_000_000.0
    };
    let max = if low { 32 } else { 128 } * 1024 * 1024;
    (
        ((rate / 8.0 * seconds * 1.25) as u64).clamp(16 * 1024 * 1024, max),
        seconds,
    )
}
impl Features {
    pub fn load(dir: &Path, probe: bool) -> Self {
        let stored = if probe {
            Value::Null
        } else {
            read(&dir.join("config/playback.json"))
        };
        Self {
            prefs: Preferences::decode(&stored),
            saved_visual: (stored["version"] == 1)
                .then(|| VisualPreferences::decode(&stored["visual"]))
                .flatten(),
            ..Default::default()
        }
    }
    pub fn save(&self, dir: &Path) -> Result<(), String> {
        let mut value = self.prefs.encode();
        if let Some(visual) = &self.saved_visual {
            value["visual"] = visual.encode();
        }
        write(&dir.join("config/playback.json"), &value)
    }
    pub fn restore_visual(&self, prefs: &mut Settings) {
        if let Some(visual) = &self.saved_visual {
            visual.restore(prefs);
        }
    }
    pub fn save_visual(&mut self, dir: &Path, prefs: &Settings) -> Result<(), String> {
        prefs.validate()?;
        let value = VisualPreferences::from_settings(prefs);
        let old = self.saved_visual.replace(value);
        if let Err(error) = self.save(dir) {
            self.saved_visual = old;
            return Err(error);
        }
        Ok(())
    }
    pub fn apply(&mut self, m: &Mpv, passthrough: u8) -> Result<(), String> {
        if self
            .applied
            .as_ref()
            .is_some_and(|(prefs, mode)| prefs == &self.prefs && *mode == passthrough)
        {
            return Ok(());
        }
        let p = &self.prefs;
        let bitmap = matches!(
            m.get("current-tracks/sub/codec").as_deref(),
            Some("hdmv_pgs_subtitle" | "pgs" | "dvd_subtitle" | "dvb_subtitle")
        );
        // ASS authoring is preserved by default, including paired 3D positions.
        for (k, v) in [
            (
                "sub-scale",
                if subtitle_is_srt(m) { p.sub_scale } else { 1.0 }.to_string(),
            ),
            ("sub-color", p.sub_color.clone()),
            (
                "sub-ass-override",
                if subtitle_is_srt(m) {
                    "scale"
                } else if bitmap {
                    "yes"
                } else {
                    "no"
                }
                .into(),
            ),
        ] {
            m.set(k, &v)?;
        }
        m.set("sub-ass-style-overrides", "")?;
        m.set("deinterlace", if p.deinterlace { "auto" } else { "no" })?;
        let filter = match (passthrough, p.audio_mode) {
            (0, 1) => Some(
                "@enigmax-night:lavfi=[acompressor=threshold=0.125:ratio=3:attack=20:release=250:makeup=1.5,alimiter=limit=0.95:level=false]",
            ),
            (0, 2) => Some(
                "@enigmax-dialogue:lavfi=[equalizer=f=1800:t=q:w=0.7:g=3,alimiter=limit=0.95:level=false]",
            ),
            _ => None,
        };
        let audio_key = (passthrough, p.audio_mode);
        if self.applied_audio != Some(audio_key) {
            // Subtitle/cursor changes never rebuild the audio filter chain.
            let filters = m.get("af").unwrap_or_default();
            for label in ["enigmax-night", "enigmax-dialogue"] {
                if filters.contains(&format!("@{label}:")) {
                    m.command(&["af", "remove", &format!("@{label}")])?;
                }
            }
            if let Some(f) = filter {
                m.command(&["af", "add", f])?;
            }
            self.applied_audio = Some(audio_key);
        }
        m.command(&[
            "script-message-to",
            "enigmax_controls",
            "stereo-cursor",
            if p.cursor { "yes" } else { "no" },
        ])?;
        self.applied = Some((self.prefs.clone(), passthrough));
        Ok(())
    }
    pub fn invalidate(&mut self) {
        self.applied = None;
    }
    pub fn poll(
        &mut self,
        m: &Mpv,
        raw: bool,
        low: bool,
        pending: bool,
        automatic: bool,
    ) -> Result<(), String> {
        if self.last_tick.elapsed().as_secs_f64() < 1.0 {
            return Ok(());
        }
        self.last_tick = Instant::now();
        if !automatic {
            return Ok(());
        }
        let active = !pending && m.get("idle-active").as_deref() == Some("no");
        let win = m.get("window-id").and_then(|v| v.parse().ok()).unwrap_or(0);
        let fullscreen = m.get("fullscreen").as_deref() == Some("yes");
        let fps = number(m, "container-fps");
        self.display
            .update(win, self.prefs.refresh && active && fullscreen, fps);
        if !active {
            return Ok(());
        }
        let stalled = m.get("paused-for-cache").as_deref() == Some("yes");
        self.status = if stalled {
            "Esperando datos: posible lectura insuficiente; no demuestra una avería".into()
        } else if number(m, "decoder-frame-drop-count") > 0.0 {
            "Se han descartado cuadros de decodificación; consulta la evolución".into()
        } else if number(m, "frame-drop-count") > 0.0 {
            "Se han descartado cuadros de presentación; consulta la evolución".into()
        } else {
            "Sin descartes acumulados ni espera de caché detectados".into()
        };
        if self.prefs.adaptive && !raw {
            if low != self.cache_low {
                self.cache_cap = 0;
                self.cache_seconds = 2.0;
                self.cache_low = low;
            }
            let bitrate = number(m, "video-bitrate") + number(m, "audio-bitrate");
            let (cap, seconds) = cache_budget(bitrate, low, stalled);
            // Hysteresis: grow during a file, reset at FILE_LOADED. No allocation
            // of raw MVC frames; this policy only bounds compressed demux packets.
            if cap > self.cache_cap || seconds > self.cache_seconds {
                self.cache_cap = cap.max(self.cache_cap);
                self.cache_seconds = seconds.max(self.cache_seconds);
                m.set("demuxer-max-bytes", &self.cache_cap.to_string())?;
                m.set("demuxer-readahead-secs", &self.cache_seconds.to_string())?;
            }
        } else if !raw {
            let cap = if low {
                16 * 1024 * 1024
            } else {
                64 * 1024 * 1024
            };
            if number(m, "demuxer-max-bytes") != cap as f64 {
                m.set("demuxer-max-bytes", &cap.to_string())?;
            }
            if self.cache_seconds != 1.0 {
                m.set("demuxer-readahead-secs", "1")?;
            }
            self.cache_cap = cap;
            self.cache_seconds = 1.0;
        }
        Ok(())
    }
    pub fn loaded(&mut self) {
        self.cache_cap = 0;
        self.cache_seconds = 2.0;
        self.applied = None;
        self.status.clear();
    }
}
fn number(m: &Mpv, k: &str) -> f64 {
    m.get(k)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite())
        .unwrap_or(0.0)
}

pub fn menu(root: &windows::Menu, s: &State) -> Result<(), String> {
    let p = &s.features.prefs;
    let menu = windows::Menu::new()?;
    menu.item(
        7000,
        "Lectura adaptativa (caché comprimida limitada)",
        p.adaptive,
    )?;
    menu.item(7001, "Igualar frecuencia de fotogramas", p.refresh)?;
    menu.item(7002, "Cursor estereoscópico", p.cursor)?;
    menu.item(7003, "Desentrelazado automático", p.deinterlace)?;
    root.submenu("Reproducción", menu)
}
pub fn audio_menu(root: &windows::Menu, s: &State) -> Result<(), String> {
    let audio = windows::Menu::new()?;
    for (i, label) in [
        "Original, sin procesar",
        "Modo nocturno (PCM)",
        "Claridad de voz: realce suave (PCM)",
    ]
    .iter()
    .enumerate()
    {
        audio.item(
            7020 + i as u32,
            label,
            s.features.prefs.audio_mode == i as u8,
        )?;
    }
    if s.passthrough != 0 {
        audio.label("Suspendidos: la salida bitstream conserva el audio original")?;
    }
    root.submenu("Filtros", audio)
}
fn subtitle_is_srt(m: &Mpv) -> bool {
    matches!(
        m.get("current-tracks/sub/codec").as_deref(),
        Some("subrip" | "srt")
    )
}
pub(crate) fn subtitle_position_supported(m: &Mpv) -> bool {
    !matches!(
        m.get("current-tracks/sub/codec").as_deref(),
        Some("ass" | "ssa")
    )
}
pub fn subtitle_menu(subs: &windows::Menu, _m: &Mpv) -> Result<(), String> {
    subs.item(7140, "Aspecto de subtítulos…", false)
}
pub fn subtitle_position_menu(m: &Mpv) -> Result<windows::Menu, String> {
    let menu = windows::Menu::new()?;
    let current = m
        .get("sub-pos")
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite())
        .unwrap_or(100.0);
    for (i, position) in crate::player::SUB_POSITIONS.iter().enumerate() {
        menu.item(
            crate::player::SUB_POSITION_FIRST + i as u32,
            &format!("{position} %"),
            (current - *position as f64).abs() < 0.01,
        )?;
    }
    Ok(menu)
}
const SUB_SCALES: [f64; 8] = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5];
fn subtitle_aspect_entries(m: &Mpv, id: u32) -> Result<Vec<crate::popup::Entry>, String> {
    if (1140..=1152).contains(&id) {
        return subtitle_position_menu(m)?.entries();
    }
    if !subtitle_is_srt(m) {
        return Err("Tamaño y color requieren una pista SRT".into());
    }
    let rows: Vec<(u32, String, bool)> = if (7100..=7107).contains(&id) {
        let scale = number(m, "sub-scale");
        SUB_SCALES
            .iter()
            .enumerate()
            .map(|(i, value)| {
                (
                    7100 + i as u32,
                    format!("{:.0} %", value * 100.0),
                    (scale - value).abs() < 0.01,
                )
            })
            .collect()
    } else if (7120..=7125).contains(&id) {
        let current = m.get("sub-color").unwrap_or_default();
        COLORS
            .iter()
            .enumerate()
            .map(|(i, (label, color))| {
                let checked = current.eq_ignore_ascii_case(color)
                    || (current.len() == 9
                        && current
                            .get(3..)
                            .is_some_and(|rgb| rgb.eq_ignore_ascii_case(&color[1..])));
                (7120 + i as u32, (*label).into(), checked)
            })
            .collect()
    } else {
        return Err("Ajuste de subtítulos inválido".into());
    };
    Ok(rows
        .into_iter()
        .map(|(id, label, checked)| {
            let mut entry = crate::popup::Entry::action(id, label);
            entry.checked = checked;
            entry
        })
        .collect())
}
fn apply_subtitle_aspect(m: &Mpv, s: &mut State, id: u32) -> Result<(), String> {
    if (1140..=1152).contains(&id) && !subtitle_position_supported(m) {
        return Ok(());
    }
    if !subtitle_aspect_entries(m, id)?.iter().any(|e| e.id == id) {
        return Err("Ajuste de subtítulos inválido".into());
    }
    if subtitle_aspect_entries(m, id)?
        .iter()
        .any(|e| e.id == id && e.checked)
    {
        return Ok(());
    }
    if (1140..=1152).contains(&id) {
        crate::player::action(m, s, id)
    } else {
        action(m, s, id)
    }
}
pub fn edit_subtitle_aspect(m: &Mpv, s: &mut State, id: u32) -> Result<bool, String> {
    if !(7140..=7142).contains(&id) {
        apply_subtitle_aspect(m, s, id)?;
    }
    let result = crate::popup::show_live_panel(
        m,
        "Aspecto de subtítulos",
        subtitle_panel_entries(m)?,
        &mut |choice| {
            if choice == 7131 {
                action(m, s, choice)?;
            } else {
                apply_subtitle_aspect(m, s, choice)?;
            }
            subtitle_panel_entries(m)
        },
    )?;
    Ok(result == crate::popup::BACK)
}
fn subtitle_panel_entries(m: &Mpv) -> Result<Vec<crate::popup::Entry>, String> {
    use crate::popup::Entry;
    let srt = subtitle_is_srt(m);
    let mut size = Entry::action(
        7140,
        if srt {
            format!("Tamaño: {:.0} %", number(m, "sub-scale") * 100.0)
        } else {
            "Tamaño: original ASS/PGS".into()
        },
    );
    size.enabled = srt;
    if srt {
        let current = number(m, "sub-scale");
        size.controls = Some([
            SUB_SCALES
                .iter()
                .rposition(|v| *v < current - 0.001)
                .map(|i| 7100 + i as u32),
            SUB_SCALES
                .iter()
                .position(|v| *v > current + 0.001)
                .map(|i| 7100 + i as u32),
            Some(7102),
        ]);
    }
    let current = number(m, "sub-pos");
    let supported = subtitle_position_supported(m);
    let mut position = Entry::action(
        7142,
        if supported {
            format!("Posición: {current:.0} %")
        } else {
            "Posición".into()
        },
    );
    position.enabled = supported;
    position.controls = Some(if supported {
        [
            crate::player::SUB_POSITIONS
                .iter()
                .rposition(|v| (*v as f64) < current - 0.001)
                .map(|i| 1140 + i as u32),
            crate::player::SUB_POSITIONS
                .iter()
                .position(|v| (*v as f64) > current + 0.001)
                .map(|i| 1140 + i as u32),
            Some(1150),
        ]
    } else {
        [None, None, None]
    });
    let palette = if srt {
        subtitle_aspect_entries(m, 7120)?
    } else {
        Vec::new()
    };
    let known_colour = palette.iter().any(|e| e.checked);
    let mut colour = Entry::action(
        7141,
        if srt && !known_colour {
            "Color: personalizado".into()
        } else if srt {
            "Color".into()
        } else {
            "Color: original ASS/PGS".into()
        },
    );
    colour.enabled = false;
    let mut result = vec![size];
    if supported {
        result.push(position);
    }
    result.push(colour);
    result.extend(palette);
    result.push(Entry::action(
        7131,
        "Restablecer todos los ajustes de subtítulos".into(),
    ));
    Ok(result)
}
const COLORS: [(&str, &str); 6] = [
    ("Blanco", "#FFFFFF"),
    ("Amarillo", "#FFFF00"),
    ("Marfil", "#FFF4D6"),
    ("Verde", "#80FF80"),
    ("Cian", "#80FFFF"),
    ("Gris claro", "#D0D0D0"),
];

pub fn action(m: &Mpv, s: &mut State, id: u32) -> Result<(), String> {
    if id == 7040 {
        return diagnostics(m, s);
    }
    if id == 7050 {
        return editions(m);
    }
    if id == 7030 || id == 7031 {
        return screen_profile(m, s, id == 7030);
    }
    let old_position = s.prefs.sub_pos;
    let old = s.features.prefs.clone();
    let p = &mut s.features.prefs;
    match id {
        7000 => p.adaptive = !p.adaptive,
        7001 => p.refresh = !p.refresh,
        7002 => p.cursor = !p.cursor,
        7003 => p.deinterlace = !p.deinterlace,
        7010..=7012 => p.track_rule = (id - 7010) as u8,
        7020..=7022 => p.audio_mode = (id - 7020) as u8,
        7100..=7107 => p.sub_scale = SUB_SCALES[(id - 7100) as usize],
        7120..=7125 => p.sub_color = COLORS[(id - 7120) as usize].1.into(),
        7131 => {
            p.sub_scale = 1.0;
            p.sub_color = "#FFFFFF".into();
            s.prefs.sub_pos = 100;
            s.applied = None;
        }
        _ => return Ok(()),
    }
    let result = s.features.apply(m, s.passthrough).and_then(|()| {
        if id == 7131 {
            crate::player::apply(m, s)?;
        }
        if s.automatic {
            if id == 7131 {
                s.save_visual()
            } else {
                s.features.save(&s.dir)
            }
        } else {
            Ok(())
        }
    });
    if let Err(e) = result {
        s.features.prefs = old;
        s.prefs.sub_pos = old_position;
        if id == 7131 {
            s.applied = None;
            let _ = crate::player::apply(m, s);
        }
        s.features.invalidate();
        let _ = s.features.apply(m, s.passthrough);
        return Err(e);
    }
    if id == 7000 && !s.features.prefs.adaptive && s.raw.is_none() {
        m.set(
            "demuxer-max-bytes",
            if s.prefs.quality == Quality::Low {
                "16MiB"
            } else {
                "64MiB"
            },
        )?;
        m.set("demuxer-readahead-secs", "1")?;
    } else if id == 7000 && s.features.prefs.adaptive {
        s.features.cache_cap = 0;
        s.features.cache_seconds = 2.0;
    }
    if s.passthrough != 0 && s.features.prefs.audio_mode != 0 {
        m.command(&[
            "show-text",
            "Efectos de audio suspendidos: bitstream intacto seleccionado",
            "4000",
        ])?;
    }
    Ok(())
}
fn editions(m: &Mpv) -> Result<(), String> {
    let rows: Value =
        serde_json::from_str(&m.get("edition-list").unwrap_or_default()).unwrap_or_default();
    let choices = rows
        .as_array()
        .into_iter()
        .flatten()
        .take(128)
        .enumerate()
        .map(|(i, v)| {
            (
                8000 + i as i32,
                v["title"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("Edición {}", i + 1)),
            )
        })
        .collect::<Vec<_>>();
    if choices.is_empty() {
        return m.command(&[
            "show-text",
            "El archivo no declara ediciones; para Blu-ray utiliza Títulos Blu-ray",
            "4000",
        ]);
    }
    let refs = choices
        .iter()
        .map(|(id, s)| (*id, s.as_str()))
        .collect::<Vec<_>>();
    let chosen = windows::choose(
        m,
        "Edición / montaje",
        "El cambio puede reiniciar la posición de reproducción.",
        &refs,
    )?;
    if choices.iter().any(|(id, _)| *id as u32 == chosen) {
        m.command_async(&["set", "edition", &(chosen - 8000).to_string()], 3)?;
    }
    Ok(())
}
fn diagnostics(m: &Mpv, s: &State) -> Result<(), String> {
    let get = |k| m.get(k).unwrap_or_else(|| "No disponible".into());
    windows::player_message(
        m,
        &format!(
            "{}\n\nCaché: {} s · {} bytes\nEspera de caché: {} · Búsqueda: {}\nDescartes: presentación {} / decodificación {}\nA/V: {} s · Pantalla: {} Hz · Vídeo: {} fps\nDecodificador: {} · Render: {}\nEntrada: {} / {} / {}\nSalida: {}\nFrecuencia automática: {}\n\nLos contadores son acumulados; no identifican por sí solos la causa física.",
            s.features.status,
            get("demuxer-cache-duration"),
            get("demuxer-cache-state/fw-bytes"),
            get("paused-for-cache"),
            get("seeking"),
            get("frame-drop-count"),
            get("decoder-frame-drop-count"),
            get("avsync"),
            get("display-fps"),
            get("container-fps"),
            get("hwdec-current"),
            get("current-vo"),
            get("video-params/primaries"),
            get("video-params/gamma"),
            get("video-params/colormatrix"),
            get("video-out-params"),
            s.features.display.status()
        ),
        false,
    );
    Ok(())
}
fn screen_key(m: &Mpv) -> String {
    let hwnd = m.get("window-id").and_then(|v| v.parse().ok()).unwrap_or(0);
    crate::display::identity(hwnd)
}
fn screen_profile(m: &Mpv, s: &State, save: bool) -> Result<(), String> {
    let key = screen_key(m);
    if key.is_empty() {
        return Err("No se pudo identificar la pantalla del reproductor".into());
    }
    let path = s.dir.join("config/screens.json");
    let mut data = read(&path);
    if !data.is_object() {
        data = json!({});
    }
    if save {
        data[&key] = json!({"hdr_sdr":s.hdr_sdr,"device":s.device,"passthrough":s.passthrough});
    } else if let Some(map) = data.as_object_mut() {
        map.remove(&key);
    }
    write(&path, &data)
}
pub fn screen_and_tracks(m: &Mpv, s: &mut State, loaded: bool) -> Result<(), String> {
    if !s.automatic || s.pending_load {
        return Ok(());
    }
    let screen = screen_key(m);
    if !screen.is_empty() && screen != s.features.screen {
        if !s.features.screen_override {
            s.features.screen_defaults =
                json!({"hdr_sdr":s.hdr_sdr,"device":s.device,"passthrough":s.passthrough});
        }
        s.features.screen = screen.clone();
        let data = read(&s.dir.join("config/screens.json"));
        let saved = data.get(&screen).cloned();
        let profile = saved.clone().or_else(|| {
            s.features
                .screen_override
                .then(|| s.features.screen_defaults.clone())
        });
        s.features.screen_override = saved.is_some();
        if let Some(v) = profile {
            let mode = v["passthrough"].as_u64().filter(|n| *n <= 2).unwrap_or(0) as u8;
            let device = v["device"].as_str().unwrap_or("");
            let available: Value =
                serde_json::from_str(&m.get("audio-device-list").unwrap_or_default())
                    .unwrap_or_default();
            let device = if device.is_empty()
                || available
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v["name"] == device))
            {
                device
            } else {
                ""
            };
            m.set(
                "audio-device",
                if device.is_empty() { "auto" } else { device },
            )?;
            s.device = device.to_owned();
            m.set("audio-exclusive", if mode == 0 { "no" } else { "yes" })?;
            m.set(
                "audio-spdif",
                ["", "ac3,dts", "ac3,eac3,dts,dts-hd,truehd"][mode as usize],
            )?;
            s.passthrough = mode;
            s.hdr_sdr = v["hdr_sdr"].as_bool().unwrap_or(false);

            s.applied = None;
            crate::player::apply(m, s)?;
        }
    }
    if loaded {
        // Apply only once per user file; internal MVC reloads and remembered
        // per-file choices take precedence over the global language policy.
        let key = format!("{}:{}", s.source, s.title);
        if s.features.track_key != key {
            s.features.track_key = key;
            if !s.has_saved_tracks() {
                let tracks: Value = serde_json::from_str(&m.get("track-list").unwrap_or_default())
                    .unwrap_or_default();
                if let Some(rows) = tracks.as_array() {
                    for (property, id) in select_tracks(rows, s.features.prefs.track_rule) {
                        m.set(property, &id.to_string())?;
                    }
                }
            }
        }
    }
    Ok(())
}
fn select_tracks(rows: &[Value], rule: u8) -> Vec<(&'static str, i64)> {
    if rule == 0 {
        return vec![];
    }
    let spanish = |v: &&Value| {
        v["lang"].as_str().is_some_and(|s| {
            matches!(
                s.to_ascii_lowercase().as_str(),
                "es" | "spa" | "esp" | "es-es"
            )
        })
    };
    let audio = rows
        .iter()
        .filter(|v| v["type"] == "audio" && v["commentary"] != true)
        .collect::<Vec<_>>();
    let aid = if rule == 1 {
        audio.iter().copied().find(spanish)
    } else {
        audio.iter().copied().find(|v| v["original"] == true)
    }
    .and_then(|v| v["id"].as_i64());
    let sub = rows
        .iter()
        .filter(|v| v["type"] == "sub")
        .filter(spanish)
        .find(|v| rule == 2 || v["forced"] == true)
        .and_then(|v| v["id"].as_i64());
    let mut result = vec![];
    if let Some(id) = aid {
        result.push(("aid", id));
    }
    // OPT_TRACKCHOICE: -2 is disabled; -1 would select a track automatically.
    result.push(("sid", sub.unwrap_or(-2)));
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ass_position_is_disabled_and_srt_pgs_position_moves_rendered_pixels() {
        use cineplayer_core::stereo::{Delivery, Layout, Output};
        use std::time::Duration;
        fn spin(m: &mut Mpv, s: &mut State, seconds: f64) {
            let until = Instant::now() + Duration::from_secs_f64(seconds);
            while Instant::now() < until {
                let event = m.wait(0.02);
                let (id, cookie) = (event.id, event.userdata);
                if id == 5 && cookie & (1_u64 << 63) != 0 {
                    m.seek_reply(cookie).unwrap();
                }
                if id == 8 {
                    s.pending_load = false;
                }
                let changed = (id == 8 || id == 22) && crate::player::subtitle_event(m, s).unwrap();
                if !s.pending_load && (id == 8 || id == 17 || changed) {
                    crate::player::apply(m, s).unwrap();
                }
            }
        }
        fn center(m: &Mpv) -> f64 {
            let (w, h, pixels) = m.screenshot_pixels().unwrap();
            let mut sum = 0_u64;
            let mut count = 0_u64;
            for (index, p) in pixels.as_chunks::<4>().0.iter().enumerate() {
                if p[..3].iter().any(|v| *v > 180) {
                    sum += (index / w) as u64;
                    count += 1;
                }
            }
            assert!(count > 30, "Missing rendered subtitle pixels");
            sum as f64 / count as f64 / h as f64
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        crate::configure(&m, true, root).unwrap();
        m.option("autofit", "640x360").unwrap();
        m.init().unwrap();
        let mut s = State::new(temp.path(), true);
        s.prefs.layout = Layout::Mono;
        crate::player::load(
            &m,
            &mut s,
            root.join("verificacion/muestras/depth-qa-pgs.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while s.pending_load {
            spin(&mut m, &mut s, 0.03);
            assert!(Instant::now() < deadline);
        }
        let mut tracks = vec![("pgs", "1".to_owned())];
        for ext in ["srt", "ass"] {
            m.command(&[
                "sub-add",
                root.join(format!("verificacion/muestras/depth-qa.{ext}"))
                    .to_str()
                    .unwrap(),
                "select",
            ])
            .unwrap();
            spin(&mut m, &mut s, 0.2);
            tracks.push((ext, m.get("sid").unwrap()));
        }
        s.automatic = true;
        for (kind, sid) in tracks.iter().rev() {
            m.set("sid", sid).unwrap();
            s.features.invalidate();
            spin(&mut m, &mut s, 0.2);
            for (layout, output, delivery) in [
                (Layout::Mono, Output::Left, Delivery::Native),
                (Layout::FullSbs, Output::Original, Delivery::Native),
                (Layout::HalfSbs, Output::Original, Delivery::Native),
                (Layout::FullOu, Output::Original, Delivery::Native),
                (Layout::HalfOu, Output::Original, Delivery::Native),
                (Layout::FullSbs, Output::Anaglyph, Delivery::Native),
                (Layout::FullSbs, Output::Original, Delivery::Anamorphic),
                (Layout::FullSbs, Output::Original, Delivery::FramePack),
            ] {
                s.prefs.layout = layout;
                s.prefs.output = output;
                s.prefs.delivery = delivery;
                m.set("pause", "yes").unwrap();
                m.seek(0.5, true).unwrap();
                spin(&mut m, &mut s, 0.2);
                action(&m, &mut s, 7131).unwrap();
                crate::player::apply(&m, &mut s).unwrap();
                m.set("pause", "no").unwrap();
                spin(&mut m, &mut s, 0.08);
                m.set("pause", "yes").unwrap();
                spin(&mut m, &mut s, 0.15);
                let entries = subtitle_panel_entries(&m).unwrap();
                let position = entries.iter().find(|e| e.id == 7142);
                assert!(
                    entries.iter().all(|e| e.swatch.is_none()),
                    "Repeated colour swatch"
                );
                let source = m.get("video-dec-params");
                let audio = m.get("aid");
                if *kind == "ass" {
                    assert!(position.is_none(), "ASS position must be hidden");
                    let before = m.get("sub-pos");
                    crate::player::action(&m, &mut s, 1148).unwrap();
                    apply_subtitle_aspect(&m, &mut s, 1148).unwrap();
                    assert_eq!(m.get("sub-pos"), before);
                    assert_eq!(s.prefs.sub_pos, 100);
                } else {
                    let position = position.unwrap();
                    assert!(position.enabled);
                    let original = center(&m);
                    m.set("pause", "no").unwrap();
                    apply_subtitle_aspect(&m, &mut s, 1146).unwrap();
                    spin(&mut m, &mut s, 0.15);
                    m.set("pause", "yes").unwrap();
                    spin(&mut m, &mut s, 0.1);
                    let raised = center(&m);
                    assert!(
                        raised < original - 0.01,
                        "{kind}/{layout:?}/{output:?}/{delivery:?}: {original} -> {raised}"
                    );
                    assert_eq!(State::new(temp.path(), false).prefs.sub_pos, 80);
                    action(&m, &mut s, 7131).unwrap();
                    spin(&mut m, &mut s, 0.15);
                    assert!(
                        (center(&m) - original).abs() < 0.01,
                        "Reset did not restore pixels"
                    );
                    assert!(position.controls.is_some());
                }
                assert_eq!(m.get("video-dec-params"), source);
                assert_eq!(m.get("aid"), audio);
            }
            println!(
                "{kind}: eight output routes checked; ASS disabled or rendered position moved/reset"
            );
        }
        // Exercise every available SRT size and colour, alternating play/pause.
        m.set("sid", &tracks[1].1).unwrap();
        spin(&mut m, &mut s, 0.15);
        s.prefs.layout = Layout::Mono;
        s.prefs.output = Output::Left;
        s.prefs.delivery = Delivery::Native;
        action(&m, &mut s, 7131).unwrap();
        crate::player::apply(&m, &mut s).unwrap();
        spin(&mut m, &mut s, 0.15);
        let path = m.get("path");
        let generation = s.generation;
        let mut sizes = Vec::new();
        for (i, value) in SUB_SCALES.iter().enumerate() {
            m.set("pause", if i % 2 == 0 { "no" } else { "yes" })
                .unwrap();
            action(&m, &mut s, 7100 + i as u32).unwrap();
            spin(&mut m, &mut s, 0.12);
            assert!((number(&m, "sub-scale") - *value).abs() < 0.001);
            let (_, _, pixels) = m.screenshot_pixels().unwrap();
            sizes.push(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[..3].iter().any(|v| *v > 180))
                    .count(),
            );
        }
        assert!(
            sizes.iter().all(|v| *v > 30) && sizes[7] > sizes[0] * 2,
            "SRT sizes not rendered: {sizes:?}"
        );
        for (i, (_, hex)) in COLORS.iter().enumerate() {
            m.set("pause", if i % 2 == 0 { "no" } else { "yes" })
                .unwrap();
            action(&m, &mut s, 7120 + i as u32).unwrap();
            spin(&mut m, &mut s, 0.12);
            let n = u32::from_str_radix(&hex[1..], 16).unwrap();
            let expected = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
            let (_, _, pixels) = m.screenshot_pixels().unwrap();
            assert!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[..3].iter().zip(expected).all(|(a, b)| a.abs_diff(b) < 40))
                    .count()
                    > 20,
                "SRT colour not rendered: {hex}"
            );
            let entries = subtitle_panel_entries(&m).unwrap();
            assert_eq!(
                entries
                    .iter()
                    .filter(|e| e.checked)
                    .map(|e| e.id)
                    .collect::<Vec<_>>(),
                vec![7120 + i as u32]
            );
            assert!(entries.iter().all(|e| e.swatch.is_none()));
        }
        assert_eq!(m.get("path"), path);
        assert_eq!(s.generation, generation);
        action(&m, &mut s, 7131).unwrap();
        // Returning through ASS must not leave position disabled on SRT/PGS.
        for (_, sid) in tracks.iter().chain(tracks[..2].iter()) {
            m.set("sid", sid).unwrap();
            spin(&mut m, &mut s, 0.1);
            assert_eq!(subtitle_position_supported(&m), sid != &tracks[2].1);
        }
        m.command(&["quit"]).unwrap();
    }
    #[test]
    fn screen_profile_applies_and_restores_defaults_without_switching_monitor_modes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        crate::configure(&m, true, root).unwrap();
        m.option("autofit", "640x360").unwrap();
        m.init().unwrap();
        let mut s = State::new(dir.path(), true);
        crate::player::load(
            &m,
            &mut s,
            root.join("verificacion/muestras/color-hdr.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let until = Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if m.wait(0.02).id == 8 {
                s.pending_load = false;
            }
            if !s.pending_load && m.get("video-params/gamma").is_some() {
                break;
            }
            assert!(Instant::now() < until);
        }
        crate::player::apply(&m, &mut s).unwrap();
        let key = screen_key(&m);
        assert!(!key.is_empty(), "No monitor identity for the player window");
        let mut profiles = json!({});
        profiles[&key] = json!({"hdr_sdr":true,"device":"missing-device-for-qa","passthrough":0,"audio_delay":0.3});
        write(&dir.path().join("config/screens.json"), &profiles).unwrap();
        s.automatic = true;
        screen_and_tracks(&m, &mut s, false).unwrap();
        assert!(s.hdr_sdr);
        assert!(
            s.device.is_empty(),
            "Missing audio device must fall back to auto"
        );
        assert_eq!(
            number(&m, "audio-delay"),
            0.0,
            "Removed manual delay must not be restored by legacy profiles"
        );
        let until = Instant::now() + std::time::Duration::from_secs(5);
        while m.get("target-trc").as_deref() != Some("bt.1886") {
            m.wait(0.02);
            crate::player::apply(&m, &mut s).unwrap();
            assert!(Instant::now() < until);
        }
        write(&dir.path().join("config/screens.json"), &json!({})).unwrap();
        s.features.screen.clear();
        screen_and_tracks(&m, &mut s, false).unwrap();
        assert!(!s.hdr_sdr);
        assert_eq!(number(&m, "audio-delay"), 0.0);
        let until = Instant::now() + std::time::Duration::from_secs(5);
        while m.get("target-trc").as_deref() != Some("auto") {
            m.wait(0.02);
            crate::player::apply(&m, &mut s).unwrap();
            assert!(Instant::now() < until);
        }
        assert!(!s.features.prefs.refresh);
    }
    #[test]
    fn bounded_cache_and_invalid_preferences() {
        assert_eq!(
            Preferences::decode(
                &json!({"version":1,"sub_scale":900,"sub_color":"bad","audio_mode":12})
            ),
            Preferences::default()
        );
        for p in [
            Preferences::default(),
            Preferences {
                sub_scale: 2.5,
                sub_color: "#80FFFF".into(),
                ..Default::default()
            },
        ] {
            assert_eq!(Preferences::decode(&p.encode()), p);
        }
        assert!(cache_budget(1e12, true, true).0 <= 32 * 1024 * 1024);
        assert!(cache_budget(f64::NAN, false, true).0 <= 128 * 1024 * 1024);
        assert!(
            cache_budget(100_000_000.0, false, true).0
                > cache_budget(100_000_000.0, false, false).0
        );
    }
    #[test]
    fn language_rules_do_not_guess_original_from_spanish_or_select_unforced() {
        let rows = vec![
            json!({"id":1,"type":"audio","lang":"eng","original":true}),
            json!({"id":2,"type":"audio","lang":"spa"}),
            json!({"id":3,"type":"sub","lang":"spa"}),
            json!({"id":4,"type":"sub","lang":"spa","forced":true}),
        ];
        assert_eq!(select_tracks(&rows, 0), vec![]);
        assert_eq!(select_tracks(&rows, 1), vec![("aid", 2), ("sid", 4)]);
        assert_eq!(select_tracks(&rows, 2), vec![("aid", 1), ("sid", 3)]);
        assert_eq!(select_tracks(&rows[..3], 1), vec![("aid", 2), ("sid", -2)]);
        let no_original = vec![
            json!({"id":8,"type":"audio","lang":"de"}),
            json!({"id":9,"type":"audio","lang":"eng","original":true,"commentary":true}),
            json!({"id":10,"type":"audio","lang":"spa"}),
        ];
        assert_eq!(
            select_tracks(&no_original, 2),
            vec![("sid", -2)],
            "Do not invent original language or choose commentary"
        );
        assert_eq!(
            select_tracks(&no_original, 1),
            vec![("aid", 10), ("sid", -2)]
        );
    }
    #[test]
    fn live_audio_subtitle_cache_transitions_preserve_video_and_persist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        crate::configure(&m, true, root).unwrap();
        m.option("vo", "null").unwrap();
        m.option("pause", "no").unwrap();
        m.init().unwrap();
        let mut s = State::new(dir.path(), true);
        crate::player::load(
            &m,
            &mut s,
            root.join("verificacion/muestras/quality-pistas-qa.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let end = Instant::now() + std::time::Duration::from_secs(12);
        loop {
            let id = m.wait(0.05).id;
            if id == 8 {
                s.pending_load = false;
            }
            if !s.pending_load && m.get("video-params/gamma").is_some() {
                break;
            }
            assert!(Instant::now() < end, "media did not load");
        }
        m.observe_string("user-data/enigmax-test-coalescing", 9001)
            .unwrap();
        for i in 0..4000 {
            m.set("user-data/enigmax-test-coalescing", &i.to_string())
                .unwrap();
        }
        let until = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let event = m.wait(0.01);
            assert_ne!(
                event.id, 24,
                "Coalesced background state overflowed the client queue"
            );
            if event.id == 22 && event.userdata == 9001 {
                break;
            }
            assert!(Instant::now() < until);
        }
        assert_eq!(
            m.user_data("user-data/enigmax-test-coalescing").as_deref(),
            Some("3999")
        );
        let color = m.get("video-params");
        let codec = m.get("video-format");
        let aid = m.get("aid");
        for mode in [7021, 7022, 7020, 7021, 7020] {
            action(&m, &mut s, mode).unwrap();
            let af = m.get("af").unwrap();
            if mode == 7020 {
                assert!(!af.contains("enigmax-"), "remaining filters: {af}");
            } else {
                assert!(af.contains(if mode == 7021 {
                    "enigmax-night"
                } else {
                    "enigmax-dialogue"
                }));
            }
            let before = m.get("af");
            action(&m, &mut s, 7104).unwrap();
            action(&m, &mut s, 7121).unwrap();
            assert_eq!(m.get("af"), before, "subtitle change rebuilt audio filters");
            assert_eq!(m.get("sub-scale").unwrap().parse::<f64>().unwrap(), 1.5);
            assert_eq!(m.get("video-params"), color);
            assert_eq!(m.get("video-format"), codec);
            assert_eq!(m.get("aid"), aid);
        }
        action(&m, &mut s, 7021).unwrap();
        s.features.apply(&m, 2).unwrap();
        assert!(
            !m.get("af").unwrap().contains("enigmax-"),
            "bitstream policy retained effects"
        );
        s.features.apply(&m, 0).unwrap();
        assert!(m.get("af").unwrap().contains("enigmax-night"));
        assert!(m.get("sub-ass-style-overrides").unwrap().is_empty());
        s.features.save(dir.path()).unwrap();
        assert_eq!(Features::load(dir.path(), false).prefs, s.features.prefs);
        crate::player::action(&m, &mut s, 1149).unwrap();
        assert_eq!(m.get("sub-pos").unwrap().parse::<f64>().unwrap(), 95.0);
        action(&m, &mut s, 7131).unwrap();
        assert_eq!(s.prefs.sub_pos, 100);
        assert_eq!(m.get("sub-pos").unwrap().parse::<f64>().unwrap(), 100.0);
        assert_eq!(s.features.prefs.sub_color, "#FFFFFF");
        assert_eq!(m.get("sub-scale").unwrap().parse::<f64>().unwrap(), 1.0);
        assert_eq!(m.get("sub-ass-override").as_deref(), Some("scale"));
        s.features.last_tick = Instant::now() - std::time::Duration::from_secs(2);
        s.features.poll(&m, false, false, false, true).unwrap();
        assert!(m.get("demuxer-max-bytes").unwrap().parse::<u64>().unwrap() <= 128 * 1024 * 1024);
        s.features.last_tick = Instant::now() - std::time::Duration::from_secs(2);
        s.features.poll(&m, false, true, false, true).unwrap();
        assert!(m.get("demuxer-max-bytes").unwrap().parse::<u64>().unwrap() <= 32 * 1024 * 1024);
        m.set("demuxer-max-bytes", "8MiB").unwrap();
        s.features.last_tick = Instant::now() - std::time::Duration::from_secs(2);
        s.features.poll(&m, true, false, false, true).unwrap();
        assert_eq!(
            m.get("demuxer-max-bytes").as_deref(),
            Some("8388608"),
            "raw MVC queue grew"
        );
        s.raw = Some(crate::mvc::Info::default());
        action(&m, &mut s, 7000).unwrap();
        action(&m, &mut s, 7120).unwrap();
        assert_eq!(
            m.get("demuxer-max-bytes").as_deref(),
            Some("8388608"),
            "Disabling adaptive cache or changing subtitle color must not reset MVC's low-memory queue"
        );
        // A failed settings write must restore the live engine, not merely UI.
        s.automatic = true;
        std::fs::remove_file(dir.path().join("config/playback.json")).unwrap();
        std::fs::create_dir(dir.path().join("config/playback.json")).unwrap();
        let old = s.features.prefs.clone();
        let scale = m.get("sub-scale");
        assert!(action(&m, &mut s, 7107).is_err());
        assert_eq!(s.features.prefs, old);
        assert_eq!(m.get("sub-scale"), scale);
        m.command(&["quit"]).unwrap();
    }
}

#[cfg(test)]
mod live_subtitle_tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn aspect_lists_stay_open_update_selection_and_close_with_escape_or_outside() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        crate::configure(&m, true, root).unwrap();
        m.init().unwrap();
        m.command(&[
            "load-script",
            root.join("verificacion/tests/stereo_popup.lua")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        let mut s = State::new(temp.path(), true);
        crate::player::load(
            &m,
            &mut s,
            root.join("verificacion/muestras/pistas-qa.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(12);
        while s.pending_load
            || m.user_data("user-data/enigmax-popup-qa-ready").as_deref() != Some("yes")
        {
            if m.wait(0.02).id == 8 {
                s.pending_load = false;
            }
            assert!(Instant::now() < deadline);
        }
        let tracks: Value = serde_json::from_str(&m.get("track-list").unwrap()).unwrap();
        let sid = tracks
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["codec"] == "subrip")
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        m.set("sid", &sid.to_string()).unwrap();
        let menu = windows::Menu::new().unwrap();
        subtitle_menu(&menu, &m).unwrap();
        let entries = menu.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, 7140);
        assert!(entries[0].enabled && entries[0].children.is_empty());
        m.set("loop-file", "inf").unwrap();
        crate::player::apply(&m, &mut s).unwrap();
        m.set("pause", "no").unwrap();
        for mode in ["mono", "sbs", "ou", "framepack", "anaglyph"] {
            m.set("user-data/enigmax-projection", mode).unwrap();
            let mut plan = json!({"sequence":[7103,7104,7121,1149,1148,7102,7124,1150,7131,"@escape"],"delay":0.04,"eye":1,"panel":mode});
            if mode == "mono" {
                plan["capture"] = root
                    .join("verificacion/1.0.2/panel-subtitulos.png")
                    .to_string_lossy()
                    .into_owned()
                    .into();
            }
            m.set("user-data/enigmax-popup-test", &plan.to_string())
                .unwrap();
            let before = (
                m.get("vf"),
                m.get("af"),
                m.get("current-vo"),
                m.get("aid"),
                m.get("sid"),
            );
            edit_subtitle_aspect(&m, &mut s, 7140).unwrap();
            assert_eq!(
                (
                    m.get("vf"),
                    m.get("af"),
                    m.get("current-vo"),
                    m.get("aid"),
                    m.get("sid")
                ),
                before
            );
            assert_eq!(s.features.prefs.sub_scale, 1.0);
            assert_eq!(s.features.prefs.sub_color, "#FFFFFF");
            assert_eq!(s.prefs.sub_pos, 100);
            assert_eq!(
                m.user_data("user-data/enigmax-popup-state").as_deref(),
                Some("")
            );
            m.set("user-data/enigmax-toolbar-popup", "yes").unwrap();
            m.set("user-data/enigmax-popup-test",&json!({"sequence":[7140,7103,"@back-button",7140,7102,"@back-key",7140,"@escape"],"delay":0.04,"eye":1,"return_flow":mode}).to_string()).unwrap();
            crate::choose_track(&m, &mut s, true).unwrap();
            assert_eq!(s.features.prefs.sub_scale, 1.0);
        }

        println!(
            "Compact subtitle panel: +/- controls, colour selection and per-row/global Reset; five projections, playback preserved"
        );
    }
}
