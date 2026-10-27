use crate::{mpv::Mpv, mvc, windows};
use cineplayer_core::stereo::{self, Algorithm, Glasses, Layout, Output, Quality, Settings};
use std::path::{Path, PathBuf};

pub struct State {
    pub prefs: Settings,
    pub hdr_sdr: bool,
    pub passthrough: u8,
    pub device: String,
    pub applied: Option<String>,
    pub source: String,
    pub titles: Vec<mvc::Title>,
    pub title: i32,
    pub raw: Option<mvc::Info>,
    pub dir: PathBuf,
    pub generation: u64,
    pub pipeline: String,
    pub resume: Option<String>,
    pub subtitles: crate::subtitles::Subtitles,
    pub external_subs: Vec<String>,
    pub pending_load: bool,
    pub automatic: bool,
    pub view_choice_made: bool,
}
impl State {
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
            prefs,
            hdr_sdr: false,
            passthrough: 0,
            device: String::new(),
            applied: None,
            source: String::new(),
            titles: Vec::new(),
            title: -1,
            raw: None,
            dir: dir.into(),
            generation: 0,
            pipeline: String::new(),
            resume: None,
            subtitles: crate::subtitles::Subtitles::default(),
            external_subs: Vec::new(),
            pending_load: false,
            automatic: !probe,
            view_choice_made: false,
        }
    }
    fn shader_path(&self) -> PathBuf {
        self.dir
            .join(format!("config/session-{}.glsl", std::process::id()))
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
            stereo::detect(
                &m.get("video-dec-params/stereo-in").unwrap_or_default(),
                &self.source,
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
        let _ = std::fs::remove_file(self.shader_path());
    }
}
fn value(s: &str) -> String {
    format!("%{}%{}", s.len(), s)
}
fn wants_mvc(s: &State) -> bool {
    let paired = if s.titles.is_empty() {
        s.prefs.layout == Layout::Mvc
            || (s.prefs.layout == Layout::Auto && stereo::detect("", &s.source) == Layout::Mvc)
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
    m.reset_seeks();
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    if !absolute.exists() {
        return Err(format!("No existe: {}", absolute.display()));
    }
    let path = absolute.to_str().ok_or("Ruta Unicode inválida")?.to_owned();
    let same_source = path == s.source;
    let disc = absolute.is_dir()
        || absolute
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("iso"));
    if path != s.source {
        s.new_open();
        s.subtitles = crate::subtitles::Subtitles::default();
        s.external_subs.clear();
        s.titles = if disc {
            m.mvc()?.titles(&path)?
        } else {
            Vec::new()
        };
        s.source = path.clone();
        s.title = -1;
    }
    if disc {
        s.title = title
            .or_else(|| (s.title >= 0).then_some(s.title))
            .or_else(|| {
                s.titles
                    .iter()
                    .find(|t| t.repeated == 0 && t.stereo != 0)
                    .or_else(|| s.titles.iter().find(|t| t.repeated == 0))
                    .map(|t| t.playlist as i32)
            })
            .ok_or("No se encontró un título Blu-ray reproducible")?;
    }
    let use_mvc = wants_mvc(s);
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
            "demuxer=rawvideo,demuxer-rawvideo-w={},demuxer-rawvideo-h={},demuxer-rawvideo-mp-format={},demuxer-rawvideo-fps={:.12},audio-files={},demuxer-max-bytes=16MiB,demuxer-max-back-bytes=0,demuxer-readahead-secs=0.25",
            info.width,
            info.height,
            if info.format == 0 {
                "yuv420p"
            } else {
                "yuv420p10"
            },
            info.fps_num as f64 / info.fps_den as f64,
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
fn reload_transport(m: &Mpv, s: &mut State) -> Result<(), String> {
    let pos = m.get("time-pos");
    let paused = m.get("pause");
    let source = s.source.clone();
    load(m, s, &source, None)?;
    s.resume = pos;
    if let Some(p) = paused {
        m.set("pause", &p)?;
    }
    Ok(())
}
pub fn subtitle_event(m: &Mpv, s: &mut State) -> Result<bool, String> {
    let changed = crate::subtitles::refresh(m, s);
    if changed && !s.source.is_empty() && wants_mvc(s) != s.raw.is_some() {
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
    let key = format!(
        "{w}/{h}/{l:?}/{}/{}/{ass:?}/{canvas:?}",
        s.prefs.encode(),
        s.hdr_sdr
    );
    if s.applied.as_ref() == Some(&key) {
        return Ok(());
    }
    s.prefs.validate()?;
    let p = &s.prefs;
    let anaglyph = l.packed() && p.output == Output::Anaglyph;
    let (ew, eh, ar) = l.eye(w, h)?;
    let aspect = p.aspect.unwrap_or(ar);
    m.set("glsl-shaders", "")?;
    m.set("vf", "")?;
    m.set("video-crop", "")?;
    let renderer = if ass.is_some() { "gpu" } else { "gpu-next" };
    m.set(
        "blend-subtitles",
        if ass.is_some() { "video" } else { "no" },
    )?;
    m.set(
        "sub-ass-override",
        if ass.is_some() { "no" } else { "scale" },
    )?;
    if m.get("current-vo").as_deref() != Some(renderer) {
        m.set("vo", renderer)?;
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
    s.pipeline = "Decodificación nativa; composición GPU".into();
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
        if p.quality == Quality::Compatibility && ass.is_none() {
            if p.convergence != 0.0
                || p.left_gain != 1.0
                || p.right_gain != 1.0
                || p.crosstalk != 0.0
            {
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
            let path = s.shader_path();
            std::fs::create_dir_all(path.parent().ok_or("Ruta de shader inválida")?)
                .map_err(|e| e.to_string())?;
            std::fs::write(&path, stereo::shader(p, l, hdr || gamma == "srgb")?)
                .map_err(|e| e.to_string())?;
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
        if ass.is_some() {
            let path = s.shader_path();
            std::fs::create_dir_all(path.parent().ok_or("Ruta de shader inválida")?)
                .map_err(|e| e.to_string())?;
            std::fs::write(&path, crate::subtitles::eye_shader(l.horizontal(), second))
                .map_err(|e| e.to_string())?;
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
        m.set(
            "video-aspect-override",
            &if l.packed() && p.output == Output::Original {
                viewport_aspect.to_string()
            } else {
                p.aspect
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "no".into())
            },
        )?;
    }
    if ass.is_some() {
        s.pipeline
            .push_str(" · ASS 3D y vídeo unidos antes del shader");
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
            } else {
                s.prefs.output.label().into()
            },
            layout_label(detected)
        )
    };
    root.label(&state)?;
    root.separator();
    root.item(
        1210,
        "Ver en 2D (TV sin 3D)",
        matches!(s.prefs.output, Output::Left | Output::Right),
    )?;
    let glasses = windows::Menu::new()?;
    for (i, g) in Glasses::ALL.iter().enumerate() {
        glasses.item(
            1200 + i as u32,
            g.label(),
            s.prefs.output == Output::Anaglyph && s.prefs.glasses == *g,
        )?;
    }
    root.submenu("Ver con gafas de colores", glasses)?;
    root.item(
        1020,
        "Mostrar las dos vistas originales",
        s.prefs.output == Output::Original,
    )?;
    root.item(
        1024,
        "Invertir profundidad (si el 3D se ve al revés)",
        s.prefs.swap,
    )?;
    root.separator();
    root.submenu("Formato del archivo", layout_menu(s)?)?;
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
    let quality = windows::Menu::new()?;
    for (i, v) in Quality::ALL.iter().enumerate() {
        quality.item(1050 + i as u32, v.label(), s.prefs.quality == *v)?;
    }
    advanced.submenu("Calidad / consumo", quality)?;
    let cal = windows::Menu::new()?;
    for (id, label) in [
        (1060, "Convergencia −0,2 %"),
        (1061, "Convergencia +0,2 %"),
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
    advanced.submenu("Ajustar profundidad, brillo y subtítulos", cal)?;
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
pub fn menu(root: &windows::Menu, m: &Mpv, s: &State) -> Result<(), String> {
    root.submenu("Vídeo 3D\tCtrl+3", stereo_menu(m, s)?)?;
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
    } else if matches!(id, 1200..=1204 | 1210 | 1020..=1023 | 1082) {
        s.view_choice_made = true;
    }
    result
}

/// Ask once per user opening; decoder/transport reloads keep the choice.
pub fn ask_view(m: &Mpv, s: &mut State) -> Result<(), String> {
    if !s.automatic
        || s.view_choice_made
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
        let view = windows::Menu::new()?;
        view.label(&format!(
            "Vídeo 3D · {} · ¿Cómo quieres verlo?",
            layout_label(s.input_layout(m))
        ))?;
        view.separator();
        view.item(1210, "Ver en 2D (sin gafas)", false)?;
        for (i, glasses) in Glasses::ALL.iter().enumerate() {
            view.item(
                1200 + i as u32,
                &format!("Anaglifo · {}", glasses.label()),
                false,
            )?;
        }
        view.item(1020, "Dos vistas originales (pantalla 3D)", false)?;
        let id = view.show(m)?;
        action(m, s, if id == 0 { 1210 } else { id })
    })();
    let restore = m.set("pause", &paused);
    result.and(restore)
}
fn action_impl(m: &Mpv, s: &mut State, id: u32) -> Result<(), String> {
    let mut reload = false;
    match id {
        1200..=1204 => {
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
            s.prefs.glasses = Glasses::ALL[(id - 1200) as usize];
            s.prefs.output = Output::Anaglyph;
            reload = true;
        }
        1210 => {
            s.prefs.output = Output::Left;
            reload = true;
        }
        1000..=1006 => {
            s.prefs.layout = Layout::ALL[(id - 1000) as usize];
            reload = true;
        }
        1020..=1023 => {
            s.prefs.output = Output::ALL[(id - 1020) as usize];
            reload = true;
        }
        1024 => {
            s.prefs.swap = !s.prefs.swap;
            reload = !s.titles.is_empty() && matches!(s.prefs.output, Output::Left | Output::Right);
        }
        1030..=1034 => s.prefs.glasses = Glasses::ALL[(id - 1030) as usize],
        1040..=1043 => s.prefs.algorithm = Algorithm::ALL[(id - 1040) as usize],
        1050..=1052 => {
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
        1060..=1061 => {
            s.prefs.convergence =
                (s.prefs.convergence + if id == 1060 { -0.002 } else { 0.002 }).clamp(-0.03, 0.03)
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
            s.prefs.convergence = 0.0;
            s.prefs.left_gain = 1.0;
            s.prefs.right_gain = 1.0;
            s.prefs.crosstalk = 0.0;
        }
        1069..=1070 => {
            s.prefs.sub_pos = (s.prefs.sub_pos + if id == 1069 { -5 } else { 5 }).clamp(50, 110)
        }
        1071 => {
            windows::message(
                &format!(
                    "Convergencia: {:.1} %\nGanancias: {:.2} / {:.2}\nCompensación de fugas: {:.1} %\nPosición de subtítulos: {}\n{}",
                    s.prefs.convergence * 100.0,
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
            windows::message(&s.subtitles.status(s.prefs.ass_mode, s.effective(m)), false);
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
                s.new_open();
            }
            return load(m, s, &source, Some(title));
        }
        _ => return Ok(()),
    }
    s.applied = None;
    if reload && !s.source.is_empty() && wants_mvc(s) != s.raw.is_some() {
        reload_transport(m, s)
    } else {
        apply(m, s)
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn a_new_open_discards_an_unconsumed_transport_resume() {
        let folder = tempfile::tempdir().unwrap();
        let mut s = State::new(folder.path(), true);
        s.resume = Some("1234.5".into());
        s.new_open();
        assert!(s.resume.is_none());
    }
}
