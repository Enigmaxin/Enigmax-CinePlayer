fn now_stamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .min(u64::MAX as u128) as u64
}
fn state_lock(dir: &std::path::Path) -> Result<std::fs::File, String> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("playback.lock"))
        .map_err(|e| e.to_string())?;
    file.lock().map_err(|e| e.to_string())?;
    Ok(file)
}
use crate::{mpv::Mpv, mvc, windows};
use cineplayer_core::stereo::{self, Algorithm, Glasses, Layout, Output, Quality, Settings};
use std::path::{Path, PathBuf};

pub struct State {
    pub detection: Option<crate::detection::Worker>,
    detection_dirty: bool,
    pub prefs: Settings,
    pub hdr_sdr: bool,
    pub passthrough: u8,
    pub device: String,
    pub applied: Option<String>,
    pub source: String,
    pub display_title: String,
    pub titles: Vec<mvc::Title>,
    pub title: i32,
    pub raw: Option<mvc::Info>,
    pub disc_direct: bool,
    pub dir: PathBuf,
    pub generation: u64,
    pub pipeline: String,
    pub resume: Option<String>,
    pub subtitles: crate::subtitles::Subtitles,
    pub external_subs: Vec<String>,
    pub pending_load: bool,
    pub closing: bool,
    pub automatic: bool,
    pub view_choice_made: bool,
    shaders: cineplayer_core::shader_store::ShaderStore,
    history: cineplayer_core::history::History,
    history_key: Option<String>,
    history_asked: bool,
    history_last: Option<cineplayer_core::history::Position>,
    history_flushed: std::time::Instant,
    last_remembered: bool,
    resume_requested: bool,
    opened: u64,
    track_choice: Option<cineplayer_core::playback::Tracks>,
    tracks_restore: bool,
    tracks_updated: u64,
    resource_restore: Option<Vec<(String, String)>>,
    pub startup_choice: bool,
    startup_play: bool,
}
impl State {
    pub fn last_video(&self) -> Option<String> {
        let path = self.dir.join("config/last-video.json");
        if path.metadata().ok()?.len() > 65536 {
            return None;
        }
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let source = value["path"].as_str()?;
        (source.len() <= 32768 && Path::new(source).is_file()).then(|| source.to_owned())
    }
    pub fn remember_last(&self) -> Result<(), String> {
        if !self.automatic {
            return Ok(());
        }
        let dir = self.dir.join("config");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let _lock = state_lock(&dir)?;
        let target = dir.join("last-video.json");
        let previous = std::fs::read(&target)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        if previous
            .as_ref()
            .and_then(|v| v["opened"].as_u64())
            .is_some_and(|v| v > self.opened)
        {
            return Ok(());
        }
        let mut file = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
        use std::io::Write;
        file.write_all(
            serde_json::json!({"path": self.source,"opened":self.opened})
                .to_string()
                .as_bytes(),
        )
        .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        windows::replace_file(file.path(), &dir.join("last-video.json"))
    }
    /// FILE_LOADED may precede decoded dimensions, especially on Blu-ray.
    /// Persist once the current source actually has video, retrying on later events.
    pub fn remember_ready(&mut self, m: &Mpv) -> Result<(), String> {
        if !self.automatic
            || self.last_remembered
            || self.pending_load
            || self.source.is_empty()
            || !(m
                .get("video-params/w")
                .and_then(|v| v.parse::<u32>().ok())
                .is_some_and(|w| w > 0)
                || m.get("track-list")
                    .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
                    .and_then(|v| v.as_array().cloned())
                    .is_some_and(|rows| {
                        rows.iter()
                            .any(|r| r["type"] == "video" && r["albumart"] != true)
                    }))
        {
            return Ok(());
        }
        let Some(path) = m.get("path") else {
            return Ok(());
        };
        let own_transport = path == format!("cine-mvc://{}", self.generation)
            || path == format!("cine-disc://{}", self.generation)
            || (!self.titles.is_empty() && path == format!("bd://mpls/{:05}", self.title));
        let own_file = Path::new(&path)
            .canonicalize()
            .ok()
            .zip(Path::new(&self.source).canonicalize().ok())
            .is_some_and(|(a, b)| a == b);
        if !own_transport && !own_file {
            return Ok(());
        }
        self.remember_last()?;
        self.last_remembered = true;
        Ok(())
    }
    /// Keep a saved position across base-view/MVC reloads until it is observed.
    pub fn restore_position(&mut self, m: &Mpv) -> Result<(), String> {
        if self.pending_load {
            return Ok(());
        }
        let Some(target) = self.resume.as_deref().and_then(|v| v.parse::<f64>().ok()) else {
            return self.start_after_resume(m);
        };
        if !target.is_finite() || target < 0.0 {
            self.resume = None;
            return Ok(());
        }
        if m.get("time-pos")
            .and_then(|v| v.parse::<f64>().ok())
            .is_some_and(|v| (v - target).abs() < 0.5)
        {
            self.resume = None;
            self.resume_requested = false;
        } else if !self.resume_requested && m.get("seekable").as_deref() == Some("yes") {
            m.seek(target, true)?;
            self.resume_requested = true;
        }
        self.start_after_resume(m)
    }
    pub fn new(dir: &Path, probe: bool) -> Self {
        let prefs = if probe {
            Settings::default()
        } else {
            std::fs::read_to_string(dir.join("config/default.ini"))
                .ok()
                .and_then(|t| Settings::decode(&t).ok())
                .unwrap_or_default()
        };
        Self {
            detection: crate::detection::Worker::new(dir),
            detection_dirty: false,
            prefs,
            hdr_sdr: false,
            passthrough: 0,
            device: String::new(),
            applied: None,
            source: String::new(),
            display_title: String::new(),
            titles: Vec::new(),
            title: -1,
            raw: None,
            disc_direct: false,
            dir: dir.into(),
            generation: 0,
            pipeline: String::new(),
            resume: None,
            subtitles: crate::subtitles::Subtitles::default(),
            external_subs: Vec::new(),
            pending_load: false,
            closing: false,
            automatic: !probe,
            view_choice_made: false,
            shaders: Default::default(),
            history: if probe {
                Default::default()
            } else {
                cineplayer_core::history::History::read(&dir.join("config/resume.json"))
            },
            history_key: None,
            history_asked: false,
            history_last: None,
            history_flushed: std::time::Instant::now(),
            last_remembered: false,
            resume_requested: false,
            opened: now_stamp(),
            track_choice: None,
            tracks_restore: false,
            tracks_updated: 0,
            resource_restore: None,
            startup_choice: false,
            startup_play: false,
        }
    }
    fn profile_path(&self) -> PathBuf {
        let hash = self
            .source
            .as_bytes()
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ *b as u64).wrapping_mul(0x100000001b3)
            });
        self.dir.join(format!("config/media-{hash:016x}.ini"))
    }
    pub fn new_open(&mut self) {
        self.flush_history();
        self.last_remembered = false;
        self.startup_choice = false;
        self.startup_play = false;
        self.opened = now_stamp();
        self.track_choice = None;
        self.tracks_restore = false;
        self.tracks_updated = 0;
        self.resume_requested = false;
        self.history_key = None;
        self.history_asked = false;
        self.history_last = None;
        // A position saved for an internal transport reload belongs to that
        // opening only. A new drop/open must not inherit its pending seek.
        self.resume = None;
        if self.automatic {
            self.view_choice_made = false;
            self.prefs.layout = Layout::Auto;
            self.prefs.output = Output::Left;
            self.prefs.swap = false;
        }
    }
    pub fn input_layout(&self, m: &Mpv) -> Layout {
        if self.raw.is_some() {
            return Layout::Mvc;
        }
        if !self.titles.is_empty() {
            return if self.prefs.layout != Layout::Mono
                && self
                    .titles
                    .iter()
                    .any(|t| t.playlist as i32 == self.title && t.stereo != 0)
            {
                Layout::Mvc
            } else {
                Layout::Mono
            };
        }
        if self.prefs.layout == Layout::Auto {
            if self.detection.as_ref().is_some_and(|p| p.mvc) {
                return Layout::Mvc;
            }
            stereo::detect_stream(
                &m.get("video-dec-params/stereo-in").unwrap_or_default(),
                &self.source,
                &m.get("current-tracks/video/codec-profile")
                    .unwrap_or_default(),
            )
        } else {
            self.prefs.layout
        }
    }
    pub fn effective(&self, m: &Mpv) -> Layout {
        let input = self.input_layout(m);
        if input == Layout::Mvc && self.raw.is_none() {
            Layout::Mono
        } else {
            input
        }
    }
    fn save(&self, path: &Path) -> Result<(), String> {
        self.prefs.validate()?;
        std::fs::create_dir_all(path.parent().ok_or("Ruta de perfil inválida")?)
            .map_err(|e| e.to_string())?;
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, self.prefs.encode()).map_err(|e| e.to_string())?;
        windows::replace_file(&tmp, path)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.flush_history();
    }
}
fn value(s: &str) -> String {
    format!("%{}%{}", s.len(), s)
}
fn wants_mvc(m: &Mpv, s: &State) -> bool {
    let paired = if s.titles.is_empty() {
        s.input_layout(m) == Layout::Mvc
    } else {
        s.titles
            .iter()
            .any(|t| t.playlist as i32 == s.title && t.stereo != 0)
    };
    paired
        && s.prefs.layout != Layout::Mono
        && (s.subtitles.requested(s.prefs.ass_mode) == Some(cineplayer_core::ass3d::Packing::Sbs)
            || match s.prefs.output {
                Output::Left => s.prefs.swap,
                Output::Right => !s.prefs.swap,
                Output::Anaglyph | Output::Original => true,
            })
}
pub fn load(m: &Mpv, s: &mut State, path: &str, title: Option<i32>) -> Result<(), String> {
    s.capture_tracks(m);
    m.reset_seeks();
    s.resume_requested = false;
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    if !absolute.exists() {
        return Err(format!("No existe: {}", absolute.display()));
    }
    s.restore_resources(m)?;
    let path = absolute.to_str().ok_or("Ruta Unicode inválida")?.to_owned();
    let same_source = path == s.source;
    let disc = absolute.is_dir()
        || absolute
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("iso"));
    if path != s.source {
        s.cancel_startup(m)?;
        s.capture_position(m);
        s.new_open();
        s.subtitles = crate::subtitles::Subtitles::default();
        s.external_subs.clear();
        s.titles = if disc {
            m.mvc()?.titles(&path)?
        } else {
            Vec::new()
        };
        s.source = path.clone();
        s.display_title = cineplayer_core::media::title(&path, "");
        s.title = -1;
    }
    if s.history_key.is_none() && s.automatic {
        s.history_key = cineplayer_core::history::History::identity(&absolute);
    }
    if !same_source {
        if s.automatic {
            s.history = cineplayer_core::history::History::read(&s.dir.join("config/resume.json"));
        }
        s.track_choice = s.history_key.as_ref().and_then(|k| s.history.get_tracks(k));
        s.external_subs = s
            .track_choice
            .as_ref()
            .map(|t| {
                t.external_subs
                    .iter()
                    .filter(|p| Path::new(p).is_file())
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
    }
    s.tracks_restore = s.track_choice.is_some();
    if disc {
        s.title = title
            .or_else(|| (s.title >= 0).then_some(s.title))
            .or_else(|| s.track_choice.as_ref().map(|t| t.title).filter(|t| *t >= 0))
            .or_else(|| {
                s.titles
                    .iter()
                    .find(|t| t.repeated == 0 && t.stereo != 0)
                    .or_else(|| s.titles.iter().find(|t| t.repeated == 0))
                    .map(|t| t.playlist as i32)
            })
            .ok_or("No se encontró un título Blu-ray reproducible")?;
    }
    if let Some(probe) = &mut s.detection {
        probe.request(if disc { "" } else { &path });
    }
    let use_mvc = wants_mvc(m, s);
    // Preserve explicitly added subtitle files across a transport reload.
    let count = if same_source {
        m.get("track-list/count")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0)
            .min(256)
    } else {
        0
    };
    for i in 0..count {
        let base = format!("track-list/{i}");
        if m.get(&format!("{base}/type")).as_deref() == Some("sub")
            && m.get(&format!("{base}/external")).as_deref() == Some("yes")
            && let Some(file) = m.get(&format!("{base}/external-filename"))
            && file != s.source
            && !file.starts_with("bd://")
            && !s.external_subs.contains(&file)
        {
            s.external_subs.push(file);
        }
    }
    let subs = if s.external_subs.is_empty() {
        String::new()
    } else {
        format!(",sub-files={}", value(&s.external_subs.join(";")))
    };
    m.set("glsl-shaders", "")?;
    m.set("vf", "")?;
    m.set("video-crop", "")?;
    m.set("video-aspect-override", "no")?;
    s.applied = None;
    s.raw = None;
    s.disc_direct = false;
    s.generation += 1;
    if disc {
        m.set("bluray-device", &path)?;
    }
    if use_mvc {
        let uri = format!("cine-mvc://{}", s.generation);
        let info = m
            .mvc()?
            .prepare(&path, if disc { s.title } else { -1 }, &uri)?;
        let audio = if disc {
            format!("bd://mpls/{:05}", s.title)
        } else {
            path.clone()
        };
        let opts = format!(
            "demuxer=rawvideo,demuxer-rawvideo-w={},demuxer-rawvideo-h={},demuxer-rawvideo-mp-format={},demuxer-rawvideo-fps={:.12},audio-files={},chapters-file={},demuxer-max-bytes=16MiB,demuxer-max-back-bytes=0,demuxer-readahead-secs=0.25",
            info.width,
            info.height,
            if info.format == 0 {
                "yuv420p"
            } else {
                "yuv420p10"
            },
            info.fps_num as f64 / info.fps_den as f64,
            value(&audio),
            value(&audio)
        );
        m.command(&["loadfile", &uri, "replace", "-1", &(opts + &subs)])?;
        s.raw = Some(info);
    } else {
        let uri = if disc {
            format!("bd://mpls/{:05}", s.title)
        } else {
            path
        };
        if subs.is_empty() {
            m.command(&["loadfile", &uri, "replace"])?;
        } else {
            m.command(&[
                "loadfile",
                &uri,
                "replace",
                "-1",
                subs.trim_start_matches(','),
            ])?;
        }
    }
    s.pending_load = true;
    Ok(())
}
pub fn resume_last(m: &Mpv, s: &mut State) -> Result<(), String> {
    if s.automatic {
        s.history = cineplayer_core::history::History::read(&s.dir.join("config/resume.json"));
    }
    let path = s.last_video().ok_or("No hay un último vídeo disponible")?;
    let record = cineplayer_core::history::History::identity(Path::new(&path))
        .and_then(|key| s.history.get(&key));
    load(
        m,
        s,
        &path,
        record
            .as_ref()
            .and_then(|r| (r.title >= 0).then_some(r.title)),
    )?;
    s.history_asked = true;
    s.resume = record.map(|r| r.seconds.to_string());
    Ok(())
}
fn reload_transport(m: &Mpv, s: &mut State) -> Result<(), String> {
    let pos = s.resume.clone().or_else(|| m.get("time-pos"));
    let paused = m.get("pause");
    let source = s.source.clone();
    load(m, s, &source, None)?;
    s.resume = pos;
    if let Some(p) = paused {
        m.set("pause", &p)?;
    }
    Ok(())
}
pub fn poll_detection(m: &Mpv, s: &mut State) -> Result<(), String> {
    if let Some(probe) = &mut s.detection {
        s.detection_dirty |= probe.poll();
    }
    if !s.detection_dirty || s.pending_load {
        return Ok(());
    }
    s.detection_dirty = false;
    s.applied = None;
    if !s.source.is_empty() && wants_mvc(m, s) != s.raw.is_some() {
        reload_transport(m, s)?;
    }
    ask_view(m, s)
}
pub fn subtitle_event(m: &Mpv, s: &mut State) -> Result<bool, String> {
    let changed = crate::subtitles::refresh(m, s);
    if changed && !s.source.is_empty() && wants_mvc(m, s) != s.raw.is_some() {
        reload_transport(m, s)?;
    }
    if changed
        && s.subtitles
            .active(s.prefs.ass_mode, s.effective(m))
            .is_some()
    {
        m.command(&[
            "show-text",
            &s.subtitles.status(s.prefs.ass_mode, s.effective(m)),
            "3000",
        ])?;
    }
    Ok(changed)
}
pub fn apply(m: &Mpv, s: &mut State) -> Result<(), String> {
    if s.pending_load {
        return Ok(());
    }
    let Some(w) = m.get("video-params/w").and_then(|v| v.parse::<u32>().ok()) else {
        return Ok(());
    };
    let Some(h) = m.get("video-params/h").and_then(|v| v.parse::<u32>().ok()) else {
        return Ok(());
    };
    let l = s.effective(m);
    let ass = s.subtitles.active(s.prefs.ass_mode, l);
    let canvas = s.subtitles.header.as_ref().map(|h| h.canvas);
    // Dynamic mastering/scene luminance is consumed per frame by libplacebo;
    // it must not rebuild the filter/shader graph on every HDR scene change.
    let decoded: serde_json::Value =
        serde_json::from_str(&m.get("video-dec-params").unwrap_or_default()).unwrap_or_default();
    let color_key = [
        "pixelformat",
        "hw-pixelformat",
        "colormatrix",
        "colorlevels",
        "primaries",
        "gamma",
        "stereo-in",
        "aspect",
        "par",
    ]
    .map(|key| decoded[key].clone());
    let key = format!(
        "{w}/{h}/{l:?}/{}/{}/{ass:?}/{canvas:?}/{}/{}/{}/{}/{}",
        s.prefs.encode(),
        s.hdr_sdr,
        s.generation,
        serde_json::to_string(&color_key).unwrap_or_default(),
        m.get("sid").unwrap_or_default(),
        m.get("current-tracks/video/codec-profile")
            .unwrap_or_default(),
        m.get("current-vo").unwrap_or_default()
    );
    if s.applied.as_ref() == Some(&key) {
        return Ok(());
    }
    s.prefs.validate()?;
    let mut settings = s.prefs.clone();
    // Input order is distinct from the user's relative left/right inversion.
    if l.packed()
        && matches!(
            m.get("video-dec-params/stereo-in").as_deref(),
            Some("sbs2r" | "sbsr" | "ab2r" | "abr")
        )
    {
        settings.swap = !settings.swap;
    }
    let p = &settings;
    let anaglyph = l.packed() && p.output == Output::Anaglyph;
    let (ew, eh, ar) = l.eye(w, h)?;
    let aspect = p.aspect.unwrap_or(ar);
    let subtitle_track = m
        .get("sid")
        .is_some_and(|id| id != "no" && id != "auto" && !id.is_empty());
    let stereo_subs = l.packed() && subtitle_track;
    // Ordinary subtitles can be projected after video presentation in native
    // SBS/OU, anamorphic and 2D. Keep gpu-next's HDR color management and avoid
    // a full-video intermediate. Only eye selection of authored ASS, anaglyph
    // mixing and the frame-packing gap require fusion before the video shader.
    let preblend = stereo_subs
        && (ass.is_some()
            || p.output == Output::Anaglyph
            || (p.output == Output::Original && p.delivery == stereo::Delivery::FramePack));
    let sub_horizontal = if preblend || p.delivery == stereo::Delivery::Native {
        l.horizontal()
    } else {
        true
    };
    m.set(
        "cine-sub-layout",
        if stereo_subs && (preblend || p.output == Output::Original) {
            if sub_horizontal { "1" } else { "2" }
        } else {
            "0"
        },
    )?;
    m.set("cine-sub-packed", if ass.is_some() { "yes" } else { "no" })?;
    // Ignore obsolete profile depth: retain authored ASS positions only.
    m.set("cine-sub-depth", "0")?;
    m.set(
        "cine-sub-par",
        &(if !preblend && p.delivery == stereo::Delivery::Anamorphic {
            2.0
        } else {
            aspect * eh as f64 / ew as f64
        })
        .clamp(0.01, 100.0)
        .to_string(),
    )?;
    m.set("glsl-shaders", "")?;
    m.set("vf", "")?;
    m.set("video-crop", "")?;
    let renderer = if preblend { "gpu" } else { "gpu-next" };
    m.set("blend-subtitles", if preblend { "video" } else { "no" })?;
    m.set(
        "sub-ass-override",
        if ass.is_some() { "no" } else { "scale" },
    )?;
    if m.get("current-vo").as_deref() != Some(renderer) {
        m.set("vo", if preblend { "gpu" } else { "gpu-next,gpu" })?;
    }
    let original_aspect = m
        .get("video-dec-params/aspect")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(w as f64 / h as f64);
    let viewport_aspect = if p.output == Output::Original && l.packed() {
        // Some decoders compensate SBS/OU metadata in display dimensions.
        // With an explicit packing, keep the geometry of the packed pixels.
        aspect
            * match l {
                Layout::FullSbs | Layout::Mvc => 2.0,
                Layout::FullOu => 0.5,
                _ => 1.0,
            }
    } else if p.output == Output::Original {
        p.aspect.unwrap_or(original_aspect)
    } else {
        aspect
    };
    let sub_par = if ass.is_some() {
        canvas
            .map(|c| viewport_aspect * c.height / c.width)
            .ok_or("Falta el lienzo ASS")?
    } else {
        0.0
    };
    m.set("sub-ass-video-aspect-override", &format!("{sub_par:.12}"))?;
    m.set("sub-pos", &p.sub_pos.to_string())?;
    m.set(
        "scale",
        if p.quality == Quality::High {
            "ewa_lanczossharp"
        } else if p.quality == Quality::Low {
            "bilinear"
        } else {
            "spline36"
        },
    )?;
    m.set(
        "dscale",
        if p.quality == Quality::High {
            "mitchell"
        } else {
            "bilinear"
        },
    )?;
    let source_hdr = matches!(
        m.get("video-dec-params/gamma").as_deref(),
        Some("pq" | "hlg")
    ) || matches!(m.get("video-params/gamma").as_deref(), Some("pq" | "hlg"));
    m.set(
        "tone-mapping",
        if source_hdr {
            if !preblend && m.get("current-vo").as_deref() == Some("gpu-next") {
                "st2094-40"
            } else {
                "bt.2390"
            }
        } else {
            "auto"
        },
    )?;
    m.set("gamut-mapping-mode", "auto")?;
    m.set(
        "hdr-compute-peak",
        if p.quality == Quality::Low {
            "no"
        } else {
            "auto"
        },
    )?;
    s.apply_resources(m)?;
    m.set("inverse-tone-mapping", "no")?;
    m.set("target-peak", "auto")?;
    m.set(
        "target-trc",
        if s.hdr_sdr || anaglyph {
            "bt.1886"
        } else {
            "auto"
        },
    )?;
    m.set(
        "target-prim",
        if s.hdr_sdr || anaglyph {
            "bt.709"
        } else {
            "auto"
        },
    )?;
    s.pipeline = if s.disc_direct {
        "Disco: lectura UDF directa de flujo verificado; composición GPU"
    } else {
        "Decodificación nativa; composición GPU"
    }
    .into();
    if !l.packed() && p.output == Output::Anaglyph {
        s.pipeline = "Vídeo 2D: no hay composición anaglifa activa".into();
        m.command(&[
            "show-text",
            "No se ha confirmado un par 3D. Si es SBS/OU, selecciona su formato en el menú.",
            "7000",
        ])?;
    }
    if l.packed() && p.output == Output::Anaglyph {
        let gamma = m.get("video-params/gamma").unwrap_or_default();
        let hdr = gamma == "pq" || gamma == "hlg";
        if p.quality == Quality::Compatibility && !preblend {
            if p.left_gain != 1.0 || p.right_gain != 1.0 || p.crosstalk != 0.0 {
                return Err("Los ajustes de calibración requieren uno de los perfiles GPU".into());
            }
            let cpu_swap =
                p.swap ^ (p.glasses == Glasses::AmberBlue && p.algorithm != Algorithm::Dubois);
            let input = l.ffmpeg(cpu_swap).ok_or("Formato 3D inválido")?;
            let color = if hdr {
                "zscale=t=linear:npl=100,format=gbrpf32le,tonemap=tonemap=hable:desat=0,zscale=p=bt709:t=bt709:m=bt709:r=limited,format=yuv420p10le,"
            } else {
                ""
            };
            m.set("hwdec", "auto-copy")?;
            m.set(
                "vf",
                &format!(
                    "lavfi=[{color}setsar=1,stereo3d={input}:{},setsar={:.12}]",
                    stereo::cpu_output(p.glasses, p.algorithm),
                    aspect * eh as f64 / ew as f64
                ),
            )?;
            s.pipeline = "Anaglifo CPU de compatibilidad · RGB 8 bits".into();
        } else {
            if hdr {
                m.set("hwdec", "auto-copy")?;
                m.set("vf","lavfi=[zscale=t=linear:npl=100,format=gbrpf32le,tonemap=tonemap=hable:desat=0,zscale=p=bt709:t=iec61966-2-1:m=bt709:r=limited,format=yuv420p10le]")?;
                s.pipeline = "HDR → SDR por CPU; composición anaglifa GPU".into();
            } else {
                m.set("hwdec", "auto")?;
            }
            let path = s
                .shaders
                .path(&s.dir, stereo::shader(p, l, hdr || gamma == "srgb")?)?;
            m.command(&[
                "change-list",
                "glsl-shaders",
                "set",
                path.to_str().ok_or("Ruta de shader inválida")?,
            ])?;
        }
        m.set("video-aspect-override", &format!("{aspect:.12}"))?;
    } else if l.packed() && matches!(p.output, Output::Left | Output::Right) {
        m.set("hwdec", "auto")?;
        let second = (p.output == Output::Right) ^ p.swap;
        if preblend {
            let path = s
                .shaders
                .path(&s.dir, crate::subtitles::eye_shader(l.horizontal(), second))?;
            m.command(&[
                "change-list",
                "glsl-shaders",
                "set",
                path.to_str().ok_or("Ruta de shader inválida")?,
            ])?;
            m.set("video-aspect-override", &format!("{aspect:.12}"))?;
        } else {
            let (x, y) = if second {
                if l.horizontal() { (ew, 0) } else { (0, eh) }
            } else {
                (0, 0)
            };
            m.set(
                "video-aspect-override",
                &format!(
                    "{:.12}",
                    aspect * (w as f64 / ew as f64) / (h as f64 / eh as f64)
                ),
            )?;
            m.set("video-crop", &format!("{ew}x{eh}+{x}+{y}"))?;
        }
    } else {
        m.set("hwdec", "auto")?;
        if l.packed()
            && p.output == Output::Original
            && (p.swap
                || p.delivery == stereo::Delivery::FramePack
                || (!l.horizontal() && p.delivery != stereo::Delivery::Native))
        {
            let path = s
                .shaders
                .path(&s.dir, stereo::packed_shader(l, p.delivery, p.swap, aspect))?;
            m.command(&[
                "change-list",
                "glsl-shaders",
                "set",
                path.to_str().ok_or("Ruta de shader inválida")?,
            ])?;
        }
        m.set(
            "video-aspect-override",
            &if l.packed() && p.output == Output::Original {
                match p.delivery {
                    stereo::Delivery::FullSbs => (32.0_f64 / 9.0).to_string(),
                    stereo::Delivery::Anamorphic => (16.0_f64 / 9.0).to_string(),
                    stereo::Delivery::FramePack => (1920.0_f64 / 2205.0).to_string(),
                    stereo::Delivery::Native => viewport_aspect.to_string(),
                }
            } else {
                p.aspect
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "no".into())
            },
        )?;
    }
    if stereo_subs {
        s.pipeline
            .push_str(" · subtítulos por ojo con profundidad GPU");
        if p.quality == Quality::Compatibility && p.output == Output::Anaglyph {
            s.pipeline
                .push_str(" · GPU necesaria para conservar el ASS");
        }
    }
    // Clear only the decoded-frame layout tag: the application already
    // performs the actual projection, and mpv must not duplicate its OSD.
    m.command(&["vf", "add", "@enigmax_layout:format=stereo-in=mono"])?;
    s.applied = Some(key);
    Ok(())
}
fn layout_label(layout: Layout) -> &'static str {
    match layout {
        Layout::Auto => "Automático",
        Layout::Mono => "Vídeo 2D",
        Layout::FullSbs => "Full SBS — lado a lado",
        Layout::HalfSbs => "Half SBS — lado a lado",
        Layout::FullOu => "Full OU — arriba / abajo",
        Layout::HalfOu => "HOU — arriba / abajo",
        Layout::Mvc => "MVC — Blu-ray 3D",
    }
}
fn layout_menu(s: &State) -> Result<windows::Menu, String> {
    let menu = windows::Menu::new()?;
    for (i, layout) in Layout::ALL.iter().enumerate() {
        menu.item(
            1000 + i as u32,
            layout_label(*layout),
            s.prefs.layout == *layout,
        )?;
    }
    Ok(menu)
}
pub fn stereo_menu(m: &Mpv, s: &State) -> Result<windows::Menu, String> {
    let root = windows::Menu::new()?;
    let detected = s.input_layout(m);
    let state = if s.source.is_empty() {
        "Abre un vídeo para comenzar".into()
    } else if s.prefs.output == Output::Anaglyph && !detected.packed() {
        "Falta seleccionar el formato 3D del archivo".into()
    } else {
        format!(
            "Ahora: {} · {}",
            if s.prefs.output == Output::Anaglyph {
                format!("3D {}", s.prefs.glasses.label())
            } else if s.prefs.output == Output::Original
                && s.prefs.delivery != stereo::Delivery::Native
            {
                match s.prefs.delivery {
                    stereo::Delivery::FullSbs => "Compatibilidad: Full SBS 3840×1080",
                    stereo::Delivery::FramePack => "Frame Packing 1920×2205 (composición)",
                    _ => "Reproducir en SBS Anamórfico",
                }
                .into()
            } else {
                s.prefs.output.label().into()
            },
            layout_label(detected)
        )
    };
    root.label(&state)?;
    root.separator();
    for (id, label, checked) in [
        (
            1020,
            "3D Original",
            s.prefs.output == Output::Original && s.prefs.delivery == stereo::Delivery::Native,
        ),
        (
            1221,
            "Reproducir en SBS Anamórfico",
            s.prefs.output == Output::Original && s.prefs.delivery == stereo::Delivery::Anamorphic,
        ),
        (
            1230,
            "Reproducir en Anaglifo",
            s.prefs.output == Output::Anaglyph,
        ),
        (
            1210,
            "Reproducir en 2D",
            matches!(s.prefs.output, Output::Left | Output::Right),
        ),
        (
            1222,
            "Frame Packing\n(Experimental)",
            s.prefs.output == Output::Original && s.prefs.delivery == stereo::Delivery::FramePack,
        ),
    ] {
        root.item(id, label, checked)?;
    }
    root.separator();
    root.submenu("Formato 3D", layout_menu(s)?)?;
    let advanced = windows::Menu::new()?;
    let eye = windows::Menu::new()?;
    eye.item(1021, "Izquierdo (habitual)", s.prefs.output == Output::Left)?;
    eye.item(1022, "Derecho", s.prefs.output == Output::Right)?;
    advanced.submenu("Ojo que mostrar en 2D", eye)?;
    let algorithm = windows::Menu::new()?;
    for (i, v) in Algorithm::ALL.iter().enumerate() {
        algorithm.item(1040 + i as u32, v.label(), s.prefs.algorithm == *v)?;
    }
    advanced.submenu("Método de color", algorithm)?;

    let cal = windows::Menu::new()?;
    for (id, label) in [
        (1062, "Ganancia izquierda −5 %"),
        (1063, "Ganancia izquierda +5 %"),
        (1064, "Ganancia derecha −5 %"),
        (1065, "Ganancia derecha +5 %"),
        (1066, "Compensación de fugas −1 %"),
        (1067, "Compensación de fugas +1 %"),
        (1068, "Restablecer calibración"),
        (1069, "Subtítulos: subir"),
        (1070, "Subtítulos: bajar"),
        (1071, "Ver ajustes actuales"),
    ] {
        cal.item(id, label, false)?;
    }
    advanced.submenu("Ajustar brillo y posición de subtítulos", cal)?;
    let subs = windows::Menu::new()?;
    for (i, mode) in cineplayer_core::ass3d::Mode::ALL.iter().enumerate() {
        subs.item(1100 + i as u32, mode.label(), s.prefs.ass_mode == *mode)?;
    }
    subs.separator();
    subs.item(1104, "Información de detección ASS", false)?;
    advanced.submenu("Subtítulos 3D", subs)?;
    root.submenu("Avanzado", advanced)?;
    let profiles = windows::Menu::new()?;
    for (id, label) in [
        (1080, "Guardar como predeterminado"),
        (1081, "Guardar perfil para este archivo"),
        (1082, "Cargar perfil de este archivo"),
        (1083, "Restablecer opciones 3D"),
    ] {
        profiles.item(id, label, false)?;
    }
    root.submenu("Ajustes guardados", profiles)?;
    Ok(root)
}
pub fn quality_menu(s: &State) -> Result<windows::Menu, String> {
    let menu = windows::Menu::new()?;
    for (i, v) in Quality::ALL.iter().enumerate() {
        menu.item(1050 + i as u32, v.label(), s.prefs.quality == *v)?;
    }
    Ok(menu)
}
pub fn menu(root: &windows::Menu, m: &Mpv, s: &State, include_stereo: bool) -> Result<(), String> {
    if include_stereo {
        root.submenu("Vídeo 3D\tCtrl+3", stereo_menu(m, s)?)?;
    }
    if !s.titles.is_empty() {
        let titles = windows::Menu::new()?;
        for (page, chunk) in s.titles.chunks(20).enumerate() {
            let group = windows::Menu::new()?;
            for (i, t) in chunk.iter().enumerate() {
                let secs = t.duration.round() as u64;
                group.item(
                    2000 + (page * 20 + i) as u32,
                    &format!(
                        "{:05} · {}:{:02}:{:02} · {} clips{}{}",
                        t.playlist,
                        secs / 3600,
                        secs / 60 % 60,
                        secs % 60,
                        t.clips,
                        if t.stereo != 0 { " · 3D" } else { "" },
                        if t.repeated != 0 {
                            " · repeticiones"
                        } else {
                            ""
                        }
                    ),
                    s.title == t.playlist as i32,
                )?;
            }
            titles.submenu(
                &format!("Títulos {}–{}", page * 20 + 1, page * 20 + chunk.len()),
                group,
            )?;
        }
        root.submenu("Títulos Blu-ray (sin menús BD-J)", titles)?;
    }
    Ok(())
}
pub fn action(m: &Mpv, s: &mut State, id: u32) -> Result<(), String> {
    let old = s.prefs.clone();
    let result = action_impl(m, s, id);
    if result.is_err() {
        s.prefs = old;
        s.applied = None;
        let _ = apply(m, s);
    } else if matches!(id, 1200..=1204 | 1210 | 1220..=1222 | 1020..=1023 | 1082) {
        s.view_choice_made = true;
    }
    result
}

/// Ask once per user opening; decoder/transport reloads keep the choice.
pub fn ask_view(m: &Mpv, s: &mut State) -> Result<(), String> {
    if !s.automatic
        || s.view_choice_made
        || s.startup_choice
        || s.pending_load
        || s.source.is_empty()
        || m.get("video-params/w").is_none()
    {
        return Ok(());
    }
    let input = s.input_layout(m);
    if !input.packed()
        && !stereo::stereo_hint(
            &m.get("video-dec-params/stereo-in").unwrap_or_default(),
            &s.source,
        )
        && s.subtitles.requested(s.prefs.ass_mode).is_none()
    {
        return Ok(());
    }
    s.view_choice_made = true;
    m.set("user-data/enigmax-toolbar-popup", "no")?;
    let paused = m.get("pause").unwrap_or_else(|| "no".into());
    m.set("pause", "yes")?;
    let result = (|| {
        if !input.packed() {
            let formats = windows::Menu::new()?;
            formats.label("Posible vídeo 3D: confirma el formato del archivo")?;
            formats.separator();
            for (i, layout) in Layout::ALL
                .iter()
                .enumerate()
                .filter(|(_, l)| **l != Layout::Auto)
            {
                formats.item(1000 + i as u32, layout_label(*layout), false)?;
            }
            let id = formats.show(m)?;
            if id == 0 {
                return Ok(());
            }
            s.prefs.layout = *Layout::ALL
                .get((id - 1000) as usize)
                .ok_or("Formato inválido")?;
            if s.prefs.layout == Layout::Mono {
                return apply(m, s);
            }
        }
        loop {
            let id = view_dialog(m, s)?;
            if id == 1232 {
                let format = layout_menu(s)?.show(m)?;
                if format != 0 {
                    action(m, s, format)?;
                }
                continue;
            }
            if id == 1233 {
                let option = stereo_menu(m, s)?.show(m)?;
                if option == 0 {
                    continue;
                }
                return action(m, s, option);
            }
            let id = if id == 1230 { choose_glasses(m)? } else { id };
            if id == 1231 {
                continue;
            }
            return action(m, s, if id == 0 { 1210 } else { id });
        }
    })();
    let restore = m.set("pause", &paused);
    result.and(restore)
}
fn view_dialog(m: &Mpv, s: &State) -> Result<u32, String> {
    use crate::cinema_dialog::{self, Choice};
    cinema_dialog::show(
        m,
        "¿Cómo quieres reproducirlo?",
        &format!(
            "Formato detectado: {} · Pulsa un modo para reproducir",
            layout_label(s.input_layout(m))
        ),
        &[
            Choice {
                id: 1020,
                title: "3D Original",
                description: "",
                image: 0,
            },
            Choice {
                id: 1221,
                title: "Reproducir en SBS Anamórfico",
                description: "",
                image: 3,
            },
            Choice {
                id: 1230,
                title: "Reproducir en Anaglifo",
                description: "Gafas de colores",
                image: 2,
            },
            Choice {
                id: 1210,
                title: "Reproducir en 2D",
                description: "",
                image: 1,
            },
            Choice {
                id: 1222,
                title: "Frame Packing",
                description: "(Experimental)",
                image: 4,
            },
        ],
        false,
    )
}
pub fn choose_glasses(m: &Mpv) -> Result<u32, String> {
    use crate::cinema_dialog::{self, Choice};
    cinema_dialog::show(
        m,
        "Elige tus gafas de colores",
        "Pulsa tus gafas para reproducir; Volver regresa a los modos 3D",
        &[
            Choice {
                id: 1200,
                title: "Rojo / cian",
                description: "",
                image: 2,
            },
            Choice {
                id: 1201,
                title: "Verde / magenta",
                description: "",
                image: 2,
            },
            Choice {
                id: 1202,
                title: "Ámbar / azul",
                description: "",
                image: 2,
            },
            Choice {
                id: 1203,
                title: "Rojo / azul",
                description: "",
                image: 2,
            },
            Choice {
                id: 1204,
                title: "Rojo / verde",
                description: "",
                image: 2,
            },
        ],
        true,
    )
}
/// Reopen the visual flow during playback, retaining pause and previous mode on cancel.
pub fn choose_view(m: &Mpv, s: &mut State) -> Result<(), String> {
    m.set("user-data/enigmax-toolbar-popup", "yes")?;
    let paused = m.get("pause").unwrap_or_else(|| "no".into());
    m.set("pause", "yes")?;
    let result = (|| {
        loop {
            let id = view_dialog(m, s)?;
            if id == 1232 {
                let format = layout_menu(s)?.show(m)?;
                if format != 0 {
                    action(m, s, format)?;
                }
                continue;
            }
            if id == 1233 {
                let option = stereo_menu(m, s)?.show(m)?;
                if option == 0 {
                    continue;
                }
                return action(m, s, option);
            }
            let id = if id == 1230 { choose_glasses(m)? } else { id };
            if id == 1231 {
                continue;
            }
            return if id == 0 { Ok(()) } else { action(m, s, id) };
        }
    })();
    let restored = m.set("pause", &paused);
    let cleared = m.set("user-data/enigmax-toolbar-popup", "no");
    result.and(restored).and(cleared)
}
fn action_impl(m: &Mpv, s: &mut State, id: u32) -> Result<(), String> {
    if id == 1230 {
        let paused = m.get("pause").unwrap_or_else(|| "no".into());
        m.set("pause", "yes")?;
        let result = choose_glasses(m).and_then(|selected| {
            if selected == 1231 {
                choose_view(m, s)
            } else if selected == 0 {
                Ok(())
            } else {
                action(m, s, selected)
            }
        });
        let restored = m.set("pause", &paused);
        return result.and(restored);
    }
    let mut reload = false;
    if matches!(id,1200..=1204 | 1220..=1222) {
        let disc_3d = s
            .titles
            .iter()
            .any(|t| t.playlist as i32 == s.title && t.stereo != 0);
        if !s.source.is_empty() && !s.pending_load && !s.input_layout(m).packed() && !disc_3d {
            let formats = windows::Menu::new()?;
            formats.label("Elige el formato 3D que tiene tu archivo")?;
            formats.separator();
            for (i, layout) in Layout::ALL
                .iter()
                .enumerate()
                .filter(|(_, l)| l.packed() || **l == Layout::Mvc)
            {
                formats.item(1000 + i as u32, layout_label(*layout), false)?;
            }
            formats.separator();
            formats.item(0, "Cancelar", false)?;
            let choice = formats.show(m)?;
            if choice == 0 {
                return Ok(());
            }
            s.prefs.layout = *Layout::ALL
                .get((choice - 1000) as usize)
                .ok_or("Formato 3D inválido")?;
        } else if disc_3d && s.prefs.layout == Layout::Mono {
            s.prefs.layout = Layout::Auto;
        }
    }
    match id {
        1200..=1204 => {
            s.prefs.glasses = Glasses::ALL[(id - 1200) as usize];
            s.prefs.output = Output::Anaglyph;
            reload = true;
        }
        1210 => {
            s.prefs.swap = false;
            s.prefs.output = Output::Left;
            reload = true;
        }
        1220..=1222 => {
            s.prefs.output = Output::Original;
            s.prefs.delivery = if id == 1220 {
                stereo::Delivery::Native
            } else if id == 1221 {
                stereo::Delivery::Anamorphic
            } else {
                stereo::Delivery::FramePack
            };
            reload = true;
        }
        1000..=1006 => {
            s.prefs.layout = Layout::ALL[(id - 1000) as usize];
            reload = true;
        }
        1020..=1023 => {
            s.prefs.output = Output::ALL[(id - 1020) as usize];
            s.prefs.delivery = stereo::Delivery::Native;
            reload = true;
        }
        1024 => {
            s.prefs.swap = !s.prefs.swap;
            reload = !s.titles.is_empty() && matches!(s.prefs.output, Output::Left | Output::Right);
        }
        1030..=1034 => s.prefs.glasses = Glasses::ALL[(id - 1030) as usize],
        1040..=1043 => s.prefs.algorithm = Algorithm::ALL[(id - 1040) as usize],
        1050..=1053 => {
            s.prefs.quality = Quality::ALL[(id - 1050) as usize];
            if id == 1052
                && s.subtitles
                    .active(s.prefs.ass_mode, s.effective(m))
                    .is_some()
                && s.prefs.output == Output::Anaglyph
            {
                m.command(&[
                    "show-text",
                    "ASS 3D: se mantiene la composición GPU para procesar texto y vídeo juntos.",
                    "7000",
                ])?;
            }
        }
        1062..=1063 => {
            s.prefs.left_gain =
                (s.prefs.left_gain + if id == 1062 { -0.05 } else { 0.05 }).clamp(0.5, 1.5)
        }
        1064..=1065 => {
            s.prefs.right_gain =
                (s.prefs.right_gain + if id == 1064 { -0.05 } else { 0.05 }).clamp(0.5, 1.5)
        }
        1066..=1067 => {
            s.prefs.crosstalk =
                (s.prefs.crosstalk + if id == 1066 { -0.01 } else { 0.01 }).clamp(0.0, 0.15)
        }
        1068 => {
            s.prefs.left_gain = 1.0;
            s.prefs.right_gain = 1.0;
            s.prefs.crosstalk = 0.0;
        }
        1069..=1070 => {
            s.prefs.sub_pos = (s.prefs.sub_pos + if id == 1069 { -5 } else { 5 }).clamp(50, 110)
        }
        1071 => {
            windows::player_message(
                m,
                &format!(
                    "Ganancias: {:.2} / {:.2}\nCompensación de fugas: {:.1} %\nPosición de subtítulos: {}\n{}",
                    s.prefs.left_gain,
                    s.prefs.right_gain,
                    s.prefs.crosstalk * 100.0,
                    s.prefs.sub_pos,
                    s.pipeline
                ),
                false,
            );
            return Ok(());
        }
        1080 => return s.save(&s.dir.join("config/default.ini")),
        1081 => {
            if s.source.is_empty() {
                return Err("Abre un archivo antes de guardar su perfil".into());
            }
            return s.save(&s.profile_path());
        }
        1082 => {
            s.prefs = Settings::decode(
                &std::fs::read_to_string(s.profile_path()).map_err(|e| e.to_string())?,
            )?;
            reload = true;
        }
        1083 => {
            s.prefs = Settings::default();
            reload = true;
        }
        1100..=1103 => {
            s.prefs.ass_mode = cineplayer_core::ass3d::Mode::ALL[(id - 1100) as usize];
            crate::subtitles::refresh(m, s);
            reload = true;
        }
        1104 => {
            windows::player_message(
                m,
                &s.subtitles.status(s.prefs.ass_mode, s.effective(m)),
                false,
            );
            return Ok(());
        }
        2000..=6095 => {
            let title = s
                .titles
                .get((id - 2000) as usize)
                .ok_or("Título inválido")?
                .playlist as i32;
            let source = s.source.clone();
            if title != s.title {
                let waiting = s.startup_choice;
                let play = s.startup_play;
                s.new_open();
                s.startup_choice = waiting;
                s.startup_play = play;
            }
            // An explicit title selection is not a reopening of the ISO:
            // do not offer a saved title that would undo the user's selection.
            s.history_asked = true;
            return load(m, s, &source, Some(title));
        }
        _ => return Ok(()),
    }
    s.applied = None;
    if reload && !s.source.is_empty() && wants_mvc(m, s) != s.raw.is_some() {
        reload_transport(m, s)
    } else {
        apply(m, s)
    }
}

impl State {
    pub fn begin_startup(&mut self, m: &Mpv) -> Result<bool, String> {
        if self.last_video().is_none() {
            return Ok(false);
        }
        m.set("pause", "yes")?;
        resume_last(m, self)?;
        self.startup_choice = true;
        m.set("pause", "yes")?;
        m.set("user-data/enigmax-startup", "yes")?;
        Ok(true)
    }
    pub fn startup_ready(&self, m: &Mpv) -> bool {
        self.startup_choice
            && !self.pending_load
            && self.resume.is_none()
            && m.get("video-params/w").is_some()
            && m.get("time-pos").is_some()
    }
    pub fn choose_startup(&mut self, m: &Mpv, restart: bool) -> Result<bool, String> {
        if !self.startup_ready(m) {
            return Ok(false);
        }
        self.startup_choice = false;
        self.startup_play = true;
        if restart {
            self.resume = Some("0".into());
            self.resume_requested = false;
        }
        m.set("user-data/enigmax-startup", "no")?;
        Ok(true)
    }
    fn start_after_resume(&mut self, m: &Mpv) -> Result<(), String> {
        if self.startup_play && self.resume.is_none() && !self.pending_load {
            m.set("pause", "no")?;
            self.startup_play = false;
        }
        Ok(())
    }
    pub fn cancel_startup(&mut self, m: &Mpv) -> Result<(), String> {
        if self.startup_choice || self.startup_play {
            self.startup_choice = false;
            self.startup_play = false;
            m.set("user-data/enigmax-startup", "no")?;
            m.set("pause", "no")?;
        }
        Ok(())
    }
    pub fn checkpoint(&mut self, seconds: &str, duration: &str) {
        if !self.automatic
            || self.pending_load
            || self.startup_choice
            || self.startup_play
            || !self.history_asked
            || self.resume.is_some()
        {
            return;
        }
        if let (Ok(seconds), Ok(duration)) = (seconds.parse::<f64>(), duration.parse::<f64>())
            && seconds.is_finite()
            && duration.is_finite()
            && seconds >= 0.0
            && duration > 0.0
        {
            self.history_last = Some(cineplayer_core::history::Position {
                seconds,
                duration,
                title: self.title,
            });
            if self.history_flushed.elapsed().as_secs() >= 30 {
                self.flush_history();
            }
        }
    }
    fn restore_resources(&mut self, m: &Mpv) -> Result<(), String> {
        if let Some(saved) = self.resource_restore.take() {
            for (key, value) in &saved {
                if let Err(error) = m.set(key, value) {
                    self.resource_restore = Some(saved);
                    return Err(error);
                }
            }
        }
        Ok(())
    }
    fn apply_resources(&mut self, m: &Mpv) -> Result<(), String> {
        if self.prefs.quality != Quality::Low {
            return self.restore_resources(m);
        }
        if self.resource_restore.is_none() {
            let mut saved = Vec::new();
            for key in ["demuxer-max-bytes", "demuxer-max-back-bytes"] {
                saved.push((
                    key.into(),
                    m.get(key)
                        .ok_or_else(|| format!("No se pudo consultar {key}"))?,
                ));
            }
            self.resource_restore = Some(saved);
        }
        let saved = self
            .resource_restore
            .as_ref()
            .ok_or("Falta el perfil de recursos")?;
        let original = saved[0]
            .1
            .parse::<u64>()
            .map_err(|_| "Límite de búfer no válido")?;
        // Never grow an existing smaller buffer; MVC already uses a smaller queue.
        let cap = if self.raw.is_some() {
            8 * 1024 * 1024
        } else {
            16 * 1024 * 1024
        };
        m.set("demuxer-max-bytes", &original.min(cap).to_string())?;
        m.set("demuxer-max-back-bytes", "0")
    }
    pub fn restore_tracks(&mut self, m: &Mpv) -> Result<(), String> {
        if self.pending_load || !self.tracks_restore {
            return Ok(());
        }
        let Some(list) = m
            .get("track-list")
            .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
        else {
            return Ok(());
        };
        let Some(rows) = list.as_array().filter(|r| !r.is_empty()) else {
            return Ok(());
        };
        if let Some(choice) = &self.track_choice {
            for (kind, key, selected) in [
                ("audio", "aid", &choice.audio),
                ("sub", "sid", &choice.subtitle),
            ] {
                if let Some(id) = selected.as_ref().and_then(|c| c.resolve(rows, kind)) {
                    m.set(key, &id)?;
                }
            }
        }
        self.tracks_restore = false;
        Ok(())
    }
    pub fn capture_tracks(&mut self, m: &Mpv) {
        if !self.automatic || self.pending_load || self.tracks_restore || self.history_key.is_none()
        {
            return;
        }
        let Some(list) = m
            .get("track-list")
            .and_then(|v| serde_json::from_str::<serde_json::Value>(&v).ok())
        else {
            return;
        };
        let choice = cineplayer_core::playback::Tracks::capture(
            self.title,
            &list,
            &m.get("aid").unwrap_or_default(),
            &m.get("sid").unwrap_or_default(),
        );
        if choice.is_some() && choice != self.track_choice {
            self.track_choice = choice;
            self.tracks_updated = now_stamp();
            self.flush_history();
        }
    }
    pub fn capture_position(&mut self, m: &Mpv) {
        let _ = self.remember_ready(m);
        self.capture_tracks(m);
        if let (Some(seconds), Some(duration)) = (m.get("time-pos"), m.get("duration")) {
            self.checkpoint(&seconds, &duration);
        }
    }
    pub fn flush_history(&mut self) {
        if !self.automatic {
            return;
        }
        let Some(key) = self.history_key.clone() else {
            return;
        };
        let updated = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let path = self.dir.join("config/resume.json");
        let result = (|| -> Result<(), String> {
            std::fs::create_dir_all(path.parent().ok_or("Falta config")?)
                .map_err(|e| e.to_string())?;
            let _lock = state_lock(path.parent().ok_or("Falta config")?)?;
            let mut merged = cineplayer_core::history::History::read(&path);
            if let Some(position) = self.history_last.clone() {
                merged.remember(key.clone(), position, updated);
            }
            if self.tracks_updated > 0
                && let Some(tracks) = self.track_choice.clone()
            {
                merged.remember_tracks(key, tracks, self.tracks_updated);
            }
            self.history = merged;
            let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or("Falta config")?)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            file.write_all(&self.history.encode())
                .map_err(|e| e.to_string())?;
            file.as_file().sync_all().map_err(|e| e.to_string())?;
            windows::replace_file(file.path(), &path)
        })();
        if let Err(e) = result {
            let _ = std::fs::write(self.dir.join("config/resume-error.txt"), e);
        }
        self.history_flushed = std::time::Instant::now();
    }
}
pub fn ask_resume(m: &Mpv, s: &mut State) -> Result<(), String> {
    if !s.automatic || s.history_asked || s.pending_load || m.get("video-params/w").is_none() {
        return Ok(());
    }
    s.history_asked = true;
    let record = s.history_key.as_ref().and_then(|key| s.history.get(key));
    let Some(record) = record else {
        return Ok(());
    };
    let paused = m.get("pause").unwrap_or_else(|| "no".into());
    m.set("pause", "yes")?;
    let seconds = record.seconds as u64;
    let result = (|| {
        let id = windows::choose(
            m,
            "¿Quieres continuar donde lo dejaste?",
            &format!(
                "{}\nÚltima posición: {:02}:{:02}:{:02}",
                std::path::Path::new(&s.source)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                seconds / 3600,
                (seconds / 60) % 60,
                seconds % 60
            ),
            &[
                (
                    1240,
                    "Reanudar reproducción\nContinuar desde la posición guardada",
                ),
                (1241, "Empezar de nuevo\nReproducir desde el principio"),
            ],
        )?;
        if id == 1240 {
            if record.title >= 0
                && record.title != s.title
                && s.titles.iter().any(|t| t.playlist as i32 == record.title)
            {
                let source = s.source.clone();
                load(m, s, &source, Some(record.title))?;
            }
            s.resume = Some(record.seconds.to_string());
            s.resume_requested = false;
        } else {
            m.seek(0.0, true)?;
        }
        Ok(())
    })();
    result.and(m.set("pause", &paused))
}

/// Retry once only; never decrypt, never silently substitute a different title.
pub fn fallback_disc(m: &Mpv, s: &mut State) -> Result<bool, String> {
    if s.disc_direct || s.raw.is_some() || s.titles.is_empty() {
        return Ok(false);
    }
    s.disc_direct = true;
    let uri = format!("cine-disc://{}", s.generation);
    let info = m.mvc()?.prepare_disc(&s.source, s.title, &uri)?;
    let start = info.audio_delay;
    let end = start + info.duration;
    let title = Path::new(&s.source)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Blu-ray");
    m.command(&[
        "loadfile", &uri, "replace", "-1",
        &format!("demuxer=lavf,demuxer-lavf-format=mpegts,start={start:.9},end={end:.9},force-media-title={}",value(title)),
    ])?;
    s.pending_load = true;
    s.applied = None;
    Ok(true)
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn iso_pump(m: &mut Mpv, s: &mut State, done: impl Fn(&Mpv, &State) -> bool) {
        let until = Instant::now() + Duration::from_secs(25);
        while Instant::now() < until {
            let e = m.wait(0.03);
            let (id, cookie) = (e.id, e.userdata);
            let error = if id == 7 && !e.data.is_null() {
                let end = unsafe { &*e.data.cast::<crate::mpv::EndFile>() };
                (end.reason == 4).then_some(end.error)
            } else {
                None
            };
            if id == 5 && cookie & (1_u64 << 63) != 0 {
                m.seek_reply(cookie).unwrap();
            }
            if let Some(error) = error {
                assert!(fallback_disc(m, s).unwrap(), "Playback error {error}");
                continue;
            }
            if id == 8 {
                println!(
                    "FILE_LOADED path={:?}, width={:?}, source={}",
                    m.get("path"),
                    m.get("video-params/w"),
                    s.source
                );
                s.pending_load = false;
            }
            s.restore_tracks(m).unwrap();
            if id == 8 || id == 17 {
                apply(m, s).unwrap();
            }
            s.restore_position(m).unwrap();
            s.capture_tracks(m);
            s.remember_ready(m).unwrap();
            if done(m, s) {
                return;
            }
        }
        panic!(
            "ISO timeout path={:?} time={:?} resume={:?} pending={} title={} seekable={:?} requested={} pause={:?}",
            m.get("path"),
            m.get("time-pos"),
            s.resume,
            s.pending_load,
            s.title,
            m.get("seekable"),
            s.resume_requested,
            m.get("pause")
        );
    }
    fn iso_engine(root: &Path) -> Mpv {
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        crate::configure(&m, true, root).unwrap();
        m.option("pause", "yes").unwrap();
        m.init().unwrap();
        m.register_mvc(Box::new(mvc::Registry::new(
            &root.join("runtime/cine_mvc.dll"),
        )))
        .unwrap();
        m
    }
    #[test]
    #[ignore = "Requires ENIGMAX_TEST_RESUME_ISO pointing to a clear authorized ISO"]
    fn last_iso_replaces_mkv_and_resumes_title_position_through_transport_reload() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = std::env::var("ENIGMAX_TEST_RESUME_ISO").unwrap();
        let path = std::path::absolute(path)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let temp = tempfile::tempdir().unwrap();
        let mut m = iso_engine(root);
        let mut s = State::new(temp.path(), false);
        let mkv = root.join("verificacion/muestras/fullsbs.mkv");
        load(&m, &mut s, mkv.to_str().unwrap(), None).unwrap();
        iso_pump(&mut m, &mut s, |_, s| s.last_remembered);
        assert_eq!(
            Path::new(&s.last_video().unwrap()).canonicalize().unwrap(),
            mkv.canonicalize().unwrap()
        );
        load(&m, &mut s, &path, None).unwrap();
        iso_pump(&mut m, &mut s, |_, s| s.last_remembered);
        assert_eq!(s.last_video().as_deref(), Some(path.as_str()));
        let title = s.title;
        let list: serde_json::Value = serde_json::from_str(&m.get("track-list").unwrap()).unwrap();
        let audio = list
            .as_array()
            .unwrap()
            .iter()
            .rfind(|r| r["type"] == "audio")
            .unwrap()["id"]
            .as_u64()
            .unwrap()
            .to_string();
        m.set("aid", &audio).unwrap();
        m.set("sid", "no").unwrap();
        s.capture_tracks(&m);
        let chosen = s.track_choice.clone().unwrap();
        s.history_asked = true;
        m.seek(123.5, true).unwrap();
        iso_pump(&mut m, &mut s, |m, _| {
            m.get("time-pos")
                .and_then(|x| x.parse::<f64>().ok())
                .is_some_and(|x| (x - 123.5).abs() < 0.15)
        });
        s.capture_position(&m);
        s.flush_history();
        let saved = s
            .history
            .get(&cineplayer_core::history::History::identity(Path::new(&path)).unwrap())
            .unwrap();
        assert_eq!(saved.title, title);
        assert!((saved.seconds - 123.5).abs() < 0.15);
        drop(s);
        drop(m);
        // Reopening the application uses the ISO path, never an internal URI
        // or the older MKV. Reload before applying the seek exercises the race.
        for mvc_view in [false, true] {
            let mut m = iso_engine(root);
            let mut s = State::new(temp.path(), false);
            assert!(s.begin_startup(&m).unwrap());
            assert_eq!(s.source, path);
            assert_eq!(s.title, title);
            let target = s.resume.clone();
            if mvc_view {
                s.prefs.output = Output::Anaglyph;
            }
            reload_transport(&m, &mut s).unwrap();
            assert_eq!(
                s.resume, target,
                "A pending resume was replaced by the zero position"
            );
            iso_pump(&mut m, &mut s, |m, s| {
                s.resume.is_none()
                    && m.get("time-pos")
                        .and_then(|x| x.parse::<f64>().ok())
                        .is_some_and(|x| (x - 123.5).abs() < 0.15)
            });
            assert!(s.startup_ready(&m));
            assert_eq!(m.get("pause").as_deref(), Some("yes"));
            if mvc_view {
                s.prefs.quality = Quality::Low;
                s.applied = None;
                apply(&m, &mut s).unwrap();
                assert_eq!(m.get("demuxer-max-bytes").as_deref(), Some("8388608"));
                s.prefs.quality = Quality::Balanced;
                s.applied = None;
                apply(&m, &mut s).unwrap();
                assert_eq!(m.get("demuxer-max-bytes").as_deref(), Some("16777216"));
            }
            assert_eq!(s.last_video().as_deref(), Some(path.as_str()));
            let list: serde_json::Value =
                serde_json::from_str(&m.get("track-list").unwrap()).unwrap();
            assert_eq!(
                m.get("aid"),
                chosen
                    .audio
                    .as_ref()
                    .unwrap()
                    .resolve(list.as_array().unwrap(), "audio")
            );
            assert_eq!(m.get("sid").as_deref(), Some("no"));
            assert_eq!(s.title, title);
            assert_eq!(
                s.raw.is_some(),
                mvc_view,
                "Expected a genuine MVC test source"
            );
            println!(
                "RESUME OK MVC={mvc_view} playlist={title} time={:?} source={}",
                m.get("time-pos"),
                s.source
            );
            assert!(s.choose_startup(&m, false).unwrap());
            iso_pump(&mut m, &mut s, |_, s| !s.startup_play);
            let bad = temp.path().join("missing.iso");
            assert!(load(&m, &mut s, bad.to_str().unwrap(), None).is_err());
            assert_eq!(s.last_video().as_deref(), Some(path.as_str()));
        }
    }
    #[test]
    fn startup_waits_at_saved_position_and_honors_both_choices() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = tempfile::tempdir().unwrap();
        let path = root.join("verificacion/muestras/pistas-qa.mkv");
        let key = cineplayer_core::history::History::identity(&path).unwrap();
        let mut seed = State::new(dir.path(), false);
        seed.source = path.to_str().unwrap().into();
        seed.remember_last().unwrap();
        let mut history = cineplayer_core::history::History::default();
        history.remember(
            key.clone(),
            cineplayer_core::history::Position {
                seconds: 12.0,
                duration: 30.0,
                title: -1,
            },
            1,
        );
        std::fs::write(dir.path().join("config/resume.json"), history.encode()).unwrap();
        drop(seed);
        for restart in [false, true] {
            let mut m = iso_engine(root);
            let mut s = State::new(dir.path(), false);
            assert!(s.begin_startup(&m).unwrap());
            iso_pump(&mut m, &mut s, |m, s| s.startup_ready(m));
            assert_eq!(m.get("pause").as_deref(), Some("yes"));
            assert!((m.get("time-pos").unwrap().parse::<f64>().unwrap() - 12.0).abs() < 0.2);
            s.checkpoint("0", "30");
            s.flush_history();
            assert_eq!(
                cineplayer_core::history::History::read(&dir.path().join("config/resume.json"))
                    .get(&key)
                    .unwrap()
                    .seconds,
                12.0
            );
            let mut controls = crate::controls::Controls::default();
            controls.ready();
            controls.sync(&m, &s).unwrap();
            reload_transport(&m, &mut s).unwrap();
            iso_pump(&mut m, &mut s, |m, s| s.startup_ready(m));
            controls.sync(&m, &s).unwrap();
            let (w, h, pixels) = m.screenshot_pixels_kind("window").unwrap();
            let layout = cineplayer_core::startup::Layout::new(w as i32, h as i32).unwrap();
            let r = layout.buttons[0];
            let index = ((r.y + 10) as usize * w + (r.x + 10) as usize) * 4;
            assert!(
                pixels[index..index + 3].iter().any(|v| *v > 20),
                "Startup choices disappeared after the reload"
            );
            m.command(&["keypress", "SPACE"]).unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            let mut explicit_choice = false;
            while Instant::now() < until {
                let e = m.wait(0.03);
                if e.id == 5 && e.userdata & (1_u64 << 63) != 0 {
                    let cookie = e.userdata;
                    m.seek_reply(cookie).unwrap();
                    continue;
                }
                if e.id == 16 && !e.data.is_null() {
                    let message = unsafe { &*e.data.cast::<crate::mpv::ClientMessage>() };
                    if message.count >= 2 && !message.args.is_null() {
                        let args = unsafe { std::slice::from_raw_parts(message.args, 2) };
                        if !args[0].is_null()
                            && !args[1].is_null()
                            && unsafe { std::ffi::CStr::from_ptr(args[0]) }.to_bytes()
                                == b"startup-choice"
                            && unsafe { std::ffi::CStr::from_ptr(args[1]) }.to_bytes()
                                == b"continue"
                        {
                            explicit_choice = true;
                            break;
                        }
                    }
                }
            }
            assert!(
                explicit_choice,
                "The actual Lua binding did not forward the startup choice"
            );
            assert_eq!(
                m.get("pause").as_deref(),
                Some("yes"),
                "Space must not bypass the startup choice"
            );
            assert!(s.choose_startup(&m, restart).unwrap());
            iso_pump(&mut m, &mut s, |m, s| {
                !s.startup_play && m.get("pause").as_deref() == Some("no")
            });
            let pos = m.get("time-pos").unwrap().parse::<f64>().unwrap();
            if restart {
                assert!(pos < 1.0);
            } else {
                assert!((12.0..13.0).contains(&pos));
            }
            assert!(!s.startup_choice);
            assert_eq!(
                m.user_data("user-data/enigmax-startup").as_deref(),
                Some("no")
            );
        }
        let empty = tempfile::tempdir().unwrap();
        let m = iso_engine(root);
        let mut s = State::new(empty.path(), false);
        assert!(!s.begin_startup(&m).unwrap());
        std::fs::create_dir_all(empty.path().join("config")).unwrap();
        std::fs::write(
            empty.path().join("config/last-video.json"),
            "{\"path\":\"Z:/missing/movie.mkv\"}",
        )
        .unwrap();
        assert!(!s.begin_startup(&m).unwrap());
    }
    #[test]
    fn late_old_instance_cannot_replace_newest_video_or_other_history() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.mkv");
        let second = dir.path().join("second.iso");
        std::fs::write(&first, b"first").unwrap();
        std::fs::write(&second, b"second").unwrap();
        let mut old = State::new(dir.path(), false);
        let mut new = State::new(dir.path(), false);
        old.source = first.to_str().unwrap().into();
        old.opened = 10;
        new.source = second.to_str().unwrap().into();
        new.opened = 20;
        new.remember_last().unwrap();
        old.remember_last().unwrap();
        assert_eq!(old.last_video().as_deref(), Some(new.source.as_str()));
        old.history_key = Some("first".into());
        new.history_key = Some("second".into());
        old.history_asked = true;
        new.history_asked = true;
        new.checkpoint("40", "120");
        new.flush_history();
        old.checkpoint("30", "120");
        old.flush_history();
        let fresh = State::new(dir.path(), false);
        assert_eq!(fresh.history.get("first").unwrap().seconds, 30.0);
        assert_eq!(fresh.history.get("second").unwrap().seconds, 40.0);
    }
    #[test]
    fn audio_subtitles_off_and_external_file_survive_reopening() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let dir = tempfile::tempdir().unwrap();
        let file = root.join("verificacion/muestras/pistas-qa.mkv");
        let ext = dir.path().join("external spanish.srt");
        std::fs::write(&ext, "1\n00:00:01,000 --> 00:00:04,000\nPrueba.\n").unwrap();
        let mut idle = State::new(dir.path(), false);
        for mode in 0..4 {
            let mut m = iso_engine(root);
            let mut s = State::new(dir.path(), false);
            load(&m, &mut s, file.to_str().unwrap(), None).unwrap();
            iso_pump(&mut m, &mut s, |m, s| {
                !s.pending_load && m.get("track-list/count").is_some()
            });
            if mode == 0 {
                m.set("aid", "2").unwrap();
                m.set("sid", "2").unwrap();
                s.capture_tracks(&m);
            } else {
                assert_eq!(m.get("aid").as_deref(), Some("2"));
                if mode == 1 {
                    assert_eq!(m.get("sid").as_deref(), Some("2"));
                    m.set("sid", "no").unwrap();
                    s.capture_tracks(&m);
                } else if mode == 2 {
                    assert_eq!(m.get("sid").as_deref(), Some("no"));
                    m.command(&["sub-add", ext.to_str().unwrap(), "select"])
                        .unwrap();
                    iso_pump(&mut m, &mut s, |m, _| {
                        m.get("track-list/count").as_deref() == Some("6")
                    });
                    s.capture_tracks(&m);
                } else {
                    assert_eq!(m.get("track-list/count").as_deref(), Some("6"));
                    let sid = m.get("sid").unwrap();
                    let list: serde_json::Value =
                        serde_json::from_str(&m.get("track-list").unwrap()).unwrap();
                    assert!(
                        list.as_array().unwrap().iter().any(|r| r["id"]
                            .as_u64()
                            .map(|id| id.to_string())
                            .as_deref()
                            == Some(&sid)
                            && r["external-filename"].as_str() == ext.to_str())
                    );
                }
            }
            s.capture_position(&m);
            s.flush_history();
            drop(s);
            drop(m);
        }
        let mut m = iso_engine(root);
        load(&m, &mut idle, file.to_str().unwrap(), None).unwrap();
        iso_pump(&mut m, &mut idle, |m, s| {
            !s.pending_load && m.get("track-list/count").as_deref() == Some("6")
        });
        assert_eq!(m.get("aid").as_deref(), Some("2"));
        assert_ne!(m.get("sid").as_deref(), Some("no"));
        assert_eq!(
            idle.track_choice.as_ref().unwrap().external_subs,
            vec![ext.to_str().unwrap().to_owned()]
        );
    }
    #[test]
    fn last_video_is_atomic_and_missing_or_invalid_records_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let media = dir.path().join("movie.mkv");
        std::fs::write(&media, b"fixture").unwrap();
        let mut s = State::new(dir.path(), false);
        assert!(s.last_video().is_none());
        s.source = media.to_str().unwrap().into();
        s.remember_last().unwrap();
        assert_eq!(s.last_video().as_deref(), Some(s.source.as_str()));
        let record = dir.path().join("config/last-video.json");
        std::fs::write(&record, b"broken").unwrap();
        assert!(s.last_video().is_none());
        s.remember_last().unwrap();
        std::fs::remove_file(media).unwrap();
        assert!(s.last_video().is_none());
    }
    #[test]
    fn positions_survive_atomic_save_and_completed_movies_are_forgotten() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("movie.mkv");
        std::fs::write(&path, b"synthetic fixture").unwrap();
        let key = cineplayer_core::history::History::identity(&path).unwrap();
        let mut s = State::new(folder.path(), false);
        s.history_key = Some(key.clone());
        s.history_asked = true;
        s.checkpoint("42.5", "120");
        s.flush_history();
        drop(s);
        let mut reopened = State::new(folder.path(), false);
        assert_eq!(
            reopened
                .history
                .get(&key)
                .unwrap_or_else(|| panic!(
                    "Resume missing: {:?}",
                    std::fs::read_to_string(folder.path().join("config/resume-error.txt"))
                ))
                .seconds,
            42.5
        );
        reopened.history_key = Some(key.clone());
        reopened.history_asked = true;
        reopened.checkpoint("119", "120");
        reopened.flush_history();
        drop(reopened);
        assert!(State::new(folder.path(), false).history.get(&key).is_none());
    }

    #[test]
    fn a_new_open_discards_an_unconsumed_transport_resume() {
        let folder = tempfile::tempdir().unwrap();
        let mut s = State::new(folder.path(), true);
        s.resume = Some("1234.5".into());
        s.new_open();
        assert!(s.resume.is_none());
    }
}
