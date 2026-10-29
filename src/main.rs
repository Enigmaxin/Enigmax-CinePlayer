#![windows_subsystem = "windows"]
#[cfg(test)]
mod audio_audit;
mod cinema_dialog;
mod controls;
mod detection;
mod mpv;
mod mvc;
mod player;
mod popup;
#[cfg(test)]
mod render_audit;
#[cfg(test)]
mod stability_audit;
mod subtitles;
mod sync_probe;
mod updater;
mod windows;
use cineplayer_core::stereo::{Algorithm, Glasses, Layout, Output};
use mpv::{ClientMessage, EndFile, Mpv};
use player::{State, apply};
use std::{
    ffi::CStr,
    path::PathBuf,
    time::{Duration, Instant},
};

#[cfg(test)]
const VERSION: &str = cineplayer_core::APP_VERSION;
const APP_NAME_VERSION: &str = concat!("Enigmax CinePlayer ", env!("CARGO_PKG_VERSION"));

fn configure(m: &Mpv, probe: bool, dir: &std::path::Path) -> Result<(), String> {
    for (k, v) in [
        ("config", "no"),
        ("load-scripts", "no"),
        ("osc", "no"),
        ("osd-bar", "no"),
        ("script-opts", "osc-idlescreen=no"),
        ("input-default-bindings", "yes"),
        ("input-vo-keyboard", "yes"),
        ("input-terminal", "no"),
        ("terminal", "no"),
        ("idle", "yes"),
        ("force-window", "immediate"),
        ("keep-open", "yes"),
        (
            "title",
            concat!(
                "Enigmax CinePlayer ",
                env!("CARGO_PKG_VERSION"),
                "${?media-title: — ${media-title}}"
            ),
        ),
        ("vo", "gpu-next,gpu"),
        ("gpu-api", "d3d11"),
        ("gpu-context", "d3d11"),
        ("hwdec", "auto"),
        // mpv probes safe direct methods, copy methods, then software. Preserve
        // its buffered retry logic instead of reloading the movie ourselves.
        ("hwdec-software-fallback", "3"),
        ("vd-lavc-dr", "auto"),
        ("d3d11-warp", "auto"),
        ("d3d11-exclusive-fs", "no"),
        ("target-colorspace-hint", "auto"),
        ("ao", "wasapi"),
        ("audio-channels", "auto-safe"),
        ("volume-max", "100"),
        ("cache", "auto"),
        ("demuxer-max-bytes", "64MiB"),
        ("demuxer-max-back-bytes", "8MiB"),
        ("autofit-larger", "90%x90%"),
        ("autofit-smaller", "640x360"),
        ("keepaspect", "yes"),
        ("drag-and-drop", "replace"),
        ("stop-screensaver", "yes"),
        ("sub-auto", "fuzzy"),
        ("screenshot-format", "png"),
    ] {
        m.option(k, v)?;
    }
    let cpus = std::thread::available_parallelism().map_or(1, usize::from);
    m.option(
        "vd-lavc-threads",
        &cineplayer_core::performance::decoder_threads(cpus).to_string(),
    )?;
    let script = dir.join("assets/enigmax_controls.lua");
    if !script.is_file() {
        return Err("Falta el módulo de controles inferiores".into());
    }
    m.option("scripts", script.to_str().ok_or("Ruta Unicode inválida")?)?;
    m.option("script-opts", &format!("enigmax-client={}", m.name()))?;
    if probe {
        if let Some(path) = std::env::var_os("ENIGMAX_PROBE_LOG") {
            m.option("log-file", path.to_str().ok_or("Ruta de log inválida")?)?;
            m.option("msg-level", "all=debug")?;
        }
        m.option(
            "pause",
            if std::env::var_os("ENIGMAX_PROBE_PLAY_SECONDS").is_some() {
                "no"
            } else {
                "yes"
            },
        )?;
        if std::env::var_os("ENIGMAX_PROBE_AUDIO_REAL").is_some() {
            m.option("ao", "wasapi")?;
            m.option("volume", "0")?;
        } else {
            m.option("ao", "null")?;
        }
    }
    Ok(())
}

fn bind(m: &Mpv) -> Result<(), String> {
    let name = m.name();
    for (key, action) in [
        ("m", "menu"),
        ("Ctrl+3", "stereo-menu"),
        ("MBTN_RIGHT", "menu"),
        ("Ctrl+o", "open"),
        ("Ctrl+s", "sub"),
        ("1", "original"),
        ("F1", "help"),
        ("q", "quit"),
        ("Q", "quit"),
    ] {
        m.command(&[
            "keybind",
            key,
            &format!("script-message-to {name} {action}"),
        ])?;
    }
    for (key, seek) in [
        ("LEFT", "-5"),
        ("RIGHT", "5"),
        ("UP", "60"),
        ("DOWN", "-60"),
        ("Shift+LEFT", "-1 exact"),
        ("Shift+RIGHT", "1 exact"),
        ("WHEEL_LEFT", "-10"),
        ("WHEEL_RIGHT", "10"),
    ] {
        m.command(&[
            "keybind",
            key,
            &format!("no-osd seek {seek}; script-message-to enigmax_controls show"),
        ])?;
    }
    for (key, delta) in [("9", "-2"), ("/", "-2"), ("0", "2"), ("*", "2")] {
        m.command(&[
            "keybind",
            key,
            &format!("no-osd add volume {delta}; script-message-to enigmax_controls show"),
        ])?;
    }
    Ok(())
}

fn owner(m: &Mpv) -> usize {
    m.get("window-id").and_then(|v| v.parse().ok()).unwrap_or(0)
}
fn text(m: &Mpv, s: &str) -> Result<(), String> {
    m.command(&["show-text", s, "7000"])
}
fn chapter_entries(m: &Mpv) -> Vec<(usize, f64, String)> {
    let raw = m.get("chapter-list").unwrap_or_default();
    let data: serde_json::Value = serde_json::from_str(&raw).unwrap_or_default();
    data.as_array()
        .into_iter()
        .flatten()
        .take(10000)
        .enumerate()
        .filter_map(|(i, c)| {
            let time = c
                .get("time")?
                .as_f64()
                .filter(|t| t.is_finite() && *t >= 0.0)?;
            let title = c.get("title").and_then(|t| t.as_str()).unwrap_or("");
            let title = title
                .chars()
                .filter(|c| !c.is_control())
                .take(160)
                .collect::<String>();
            Some((i, time, title))
        })
        .collect()
}
fn chapter_navigation(m: &Mpv, step: Option<i32>) -> Result<(), String> {
    let entries = chapter_entries(m);
    if entries.is_empty() {
        return text(m, "Este vídeo no contiene capítulos");
    }
    let current = m
        .get("chapter")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(-1);
    if let Some(step) = step {
        let index = (current + step).clamp(0, entries.last().map(|e| e.0 as i32).unwrap_or(0));
        return m.command_async(&["set", "chapter", &index.to_string()], 3);
    }
    let items = entries
        .iter()
        .map(|(index, time, title)| {
            let seconds = *time as u64;
            let mut entry = popup::Entry::action(
                50000 + *index as u32,
                format!(
                    "{} · {:02}:{:02}:{:02}{}{}",
                    index + 1,
                    seconds / 3600,
                    seconds / 60 % 60,
                    seconds % 60,
                    if title.is_empty() { "" } else { " · " },
                    title
                ),
            );
            entry.checked = *index as i32 == current;
            entry
        })
        .collect();
    let id = popup::show_anchored(
        m,
        "Capítulos",
        items,
        cineplayer_core::control_layout::Button::Chapters,
    )?;
    if id >= 50000 && entries.iter().any(|e| e.0 == (id - 50000) as usize) {
        m.command_async(&["set", "chapter", &(id - 50000).to_string()], 3)?;
    }
    Ok(())
}
fn help(m: &Mpv) -> Result<(), String> {
    if popup::stereo(m) {
        windows::player_message(
            m,
            &format!(
                "{APP_NAME_VERSION}\nCtrl+O: abrir vídeo\nClic derecho / M: menú general\nCtrl+3: vídeo 3D\nEspacio: pausa\nF / doble clic: pantalla completa\n−10 / +10 y flechas: buscar\n1: tamaño original\nQ: cerrar"
            ),
            false,
        );
        return Ok(());
    }
    text(
        m,
        &format!(
            "{APP_NAME_VERSION}\nArrastra un vídeo aquí o pulsa Ctrl+O\nClic derecho / M: menú general · Ctrl+3: vídeo 3D\nEspacio: pausa · F / doble clic: pantalla completa\nFlechas: avanzar/retroceder · 1: ventana al tamaño original\nF1: ayuda · Q: cerrar"
        ),
    )
}

fn runtime_error(m: &Mpv, dir: &std::path::Path, context: &str, error: &str) {
    // Keep one bounded diagnostic, never a modal dialog on the event consumer.
    let text = format!(
        "{context}: {}",
        error.chars().take(8192).collect::<String>()
    );
    let _ = std::fs::create_dir_all(dir.join("config"));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = std::fs::write(
        dir.join("config/ultimo-error-reproduccion.log"),
        format!(
            "Unix: {stamp}\n{text}\nArchivo: {}\nPosición: {}\n",
            m.get("path")
                .unwrap_or_default()
                .chars()
                .take(8192)
                .collect::<String>(),
            m.get("time-pos").unwrap_or_default()
        ),
    );
    if m.get("idle-active").as_deref() == Some("no") {
        let _ = m.command_async(&["show-text", &text, "5000"], 0);
    }
}

enum Action {
    Id(u32),
    Track(String, String),
    Device(String),
}
type TrackMenus = (windows::Menu, windows::Menu, Vec<(u32, Action)>);
fn track_menus(m: &Mpv) -> Result<TrackMenus, String> {
    let mut actions = Vec::new();
    let tracks = windows::Menu::new()?;
    let subs = windows::Menu::new()?;
    tracks.label("Audio")?;
    tracks.separator();
    subs.label("Subtítulos")?;
    subs.separator();
    subs.item(300, "Desactivados", m.get("sid").as_deref() == Some("no"))?;
    actions.push((300, Action::Track("sid".into(), "no".into())));
    // One consistent snapshot instead of up to 1,536 synchronous property
    // queries. The decoder can keep working while the native menu is open.
    let snapshot: serde_json::Value =
        serde_json::from_str(&m.get("track-list").unwrap_or_default()).unwrap_or_default();
    for (i, entry) in snapshot
        .as_array()
        .into_iter()
        .flatten()
        .take(256)
        .enumerate()
    {
        let kind = entry["type"].as_str().unwrap_or_default();
        if kind != "audio" && kind != "sub" {
            continue;
        }
        let Some(track) = entry["id"]
            .as_i64()
            .filter(|id| *id >= 0)
            .map(|id| id.to_string())
        else {
            continue;
        };
        let id = 400 + i as u32;
        let label = cineplayer_core::track_info::label(entry, &track);
        let selected = entry["selected"].as_bool().unwrap_or(false);
        if kind == "audio" {
            tracks.item(id, &label, selected)?;
        } else {
            subs.item(id, &label, selected)?;
        }
        actions.push((
            id,
            Action::Track(if kind == "audio" { "aid" } else { "sid" }.into(), track),
        ));
    }
    if !actions
        .iter()
        .any(|(_, a)| matches!(a,Action::Track(k,_) if k=="aid"))
    {
        tracks.label("Este archivo no contiene pistas de audio")?;
    }
    subs.separator();
    subs.item(2, "Añadir subtítulos…", false)?;
    subs.item(58, "Ajustar sincronización…", false)?;
    actions.extend([(2, Action::Id(2)), (58, Action::Id(58))]);
    Ok((tracks, subs, actions))
}
fn choose_track(m: &Mpv, s: &mut State, sub: bool) -> Result<(), String> {
    let (audio, subs, actions) = track_menus(m)?;
    let id = if sub { subs.show(m)? } else { audio.show(m)? };
    if let Some((_, a)) = actions.into_iter().find(|(n, _)| *n == id) {
        action(m, s, a)?;
    }
    Ok(())
}
type MenuActions = Vec<(u32, Action)>;
fn build_menu(m: &Mpv, s: &State, remaining: bool) -> Result<(windows::Menu, MenuActions), String> {
    let root = windows::Menu::new()?;
    root.item(1, "Abrir vídeo…\tCtrl+O", false)?;
    root.item(2, "Añadir subtítulos…\tCtrl+S", false)?;
    root.separator();
    player::menu(&root, m, s, !remaining)?;
    let ar = windows::Menu::new()?;
    for (id, label, aspect) in [
        (20, "Según el formato", None),
        (21, "16:9", Some(16.0 / 9.0)),
        (22, "4:3", Some(4.0 / 3.0)),
        (23, "1,85:1", Some(1.85)),
        (24, "2,35:1", Some(2.35)),
        (25, "2,39:1", Some(2.39)),
    ] {
        ar.item(id, label, s.prefs.aspect == aspect)?;
    }
    root.submenu("Relación de aspecto final", ar)?;
    let hdr = windows::Menu::new()?;
    hdr.item(
        30,
        "Automático (HDR si Windows y pantalla lo permiten)",
        !s.hdr_sdr,
    )?;
    hdr.item(31, "Convertir siempre a SDR", s.hdr_sdr)?;
    root.submenu("Color / HDR", hdr)?;
    root.submenu("Calidad / consumo", player::quality_menu(s)?)?;
    let audio = windows::Menu::new()?;
    for (id, label, value) in [
        (40, "TV / altavoces: decodificar a PCM", 0),
        (41, "Salida Óptica/Coaxial", 1),
        (42, "Salida Bitstream HDMI", 2),
    ] {
        audio.item(id, label, s.passthrough == value)?;
    }
    root.submenu("Salida de audio", audio)?;
    let mut actions = Vec::new();
    let devices = windows::Menu::new()?;
    devices.item(
        100,
        "Dispositivo predeterminado de Windows",
        s.device.is_empty(),
    )?;
    actions.push((100, Action::Device("auto".into())));
    let n = m
        .get("audio-device-list/count")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(128);
    for i in 0..n {
        if let Some(name) = m.get(&format!("audio-device-list/{i}/name")) {
            if name == "auto" {
                continue;
            }
            let label = m
                .get(&format!("audio-device-list/{i}/description"))
                .unwrap_or_else(|| name.clone());
            let id = 101 + i as u32;
            devices.item(id, &label, s.device == name)?;
            actions.push((id, Action::Device(name)));
        }
    }
    root.submenu("Dispositivo de sonido", devices)?;
    if !remaining {
        let (tracks, subs, track_actions) = track_menus(m)?;
        actions.extend(track_actions);
        root.submenu("Pista de audio", tracks)?;
        root.submenu("Pista de subtítulos", subs)?;
    }
    root.separator();
    root.item(50, "Pausa / continuar\tEspacio", false)?;
    root.item(
        51,
        "Pantalla completa\tF",
        m.get("fullscreen").as_deref() == Some("yes"),
    )?;
    root.item(52, "Ventana al tamaño original\t1", false)?;
    root.item(53, "Información de reproducción", false)?;
    root.item(54, "Ayuda\tF1", false)?;
    let about = windows::Menu::new()?;
    about.label(APP_NAME_VERSION)?;
    about.item(56, "Información del programa…", false)?;
    about.item(57, "Buscar actualizaciones…", false)?;
    root.submenu("Acerca de", about)?;
    root.item(55, "Cerrar\tQ", false)?;
    Ok((root, actions))
}
fn menu(m: &Mpv, s: &State, remaining: bool) -> Result<Option<Action>, String> {
    let (root, mut actions) = build_menu(m, s, remaining)?;
    let id = root.show(m)?;
    Ok(if id == 0 {
        None
    } else if let Some(pos) = actions.iter().position(|(n, _)| *n == id) {
        Some(actions.swap_remove(pos).1)
    } else {
        Some(Action::Id(id))
    })
}

fn open(m: &Mpv, s: &mut State, sub: bool) -> Result<(), String> {
    if let Some(path) = windows::open_file(owner(m), sub)? {
        let value = path.to_str().ok_or("Ruta no representable como Unicode")?;
        if sub {
            m.command(&["sub-add", value, "select"])?;
        } else {
            s.cancel_startup(m)?;
            s.capture_position(m);
            s.new_open();
            player::load(m, s, value, None)?;
        }
    }
    Ok(())
}
fn finish_startup(m: &Mpv, s: &mut State, restart: bool) -> Result<(), String> {
    if s.choose_startup(m, restart)? {
        player::ask_view(m, s)?;
    }
    Ok(())
}
fn original(m: &Mpv) -> Result<(), String> {
    m.set("fullscreen", "no")?;
    m.set("window-scale", "1")
}
fn info(m: &Mpv, s: &State) {
    let get = |key: &str| {
        m.get(key)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "No disponible".into())
    };
    let bytes = std::fs::metadata(&s.source)
        .ok()
        .map(|f| cineplayer_core::media::size(f.len()))
        .unwrap_or_else(|| "No disponible".into());
    let filename = cineplayer_core::media::title(&s.source, "");
    let fps = m
        .get("container-fps")
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .map(|v| {
            format!("{v:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_owned()
        })
        .unwrap_or_else(|| "No disponible".into());
    let pixels = m
        .get("video-params/hw-pixelformat")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| get("video-params/pixelformat"));
    let mut lines = vec![
        "ARCHIVO".to_owned(),
        s.display_title.clone(),
        format!(
            "{} · {} · {}",
            cineplayer_core::media::title(&s.source, ""),
            get("file-format"),
            bytes
        ),
        "VÍDEO".into(),
        format!(
            "{} · {}×{} · {} fps · {}",
            get("video-format"),
            get("video-params/w"),
            get("video-params/h"),
            fps,
            cineplayer_core::media::bitrate(m.get("video-bitrate"))
        ),
        format!(
            "Color: {} / {} / {} · {}",
            get("video-params/primaries"),
            get("video-params/gamma"),
            get("video-params/colormatrix"),
            get("video-params/colorlevels")
        ),
        format!(
            "Píxeles: {} · Decodificación: {}",
            pixels,
            get("hwdec-current")
        ),
        format!(
            "Renderizador: {} · Descartados: {}",
            get("current-vo"),
            get("frame-drop-count")
        ),
        if s.input_layout(m).packed() {
            format!(
                "3D: {} → {}",
                s.input_layout(m).label(),
                s.prefs.output.label()
            )
        } else {
            "Presentación: 2D".into()
        },
    ];
    if s.display_title.is_empty() || s.display_title == filename {
        lines.remove(1);
    }
    let tracks: serde_json::Value =
        serde_json::from_str(&m.get("track-list").unwrap_or_default()).unwrap_or_default();
    for (kind, heading) in [("audio", "AUDIOS"), ("sub", "SUBTÍTULOS")] {
        lines.push(heading.into());
        let mut found = false;
        for t in tracks
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| t["type"].as_str() == Some(kind))
        {
            found = true;
            let field = |k: &str| t[k].as_str().unwrap_or("");
            let id = t["id"].as_i64().unwrap_or(0);
            let selected = if t["selected"].as_bool() == Some(true) {
                "●"
            } else {
                "○"
            };
            let mut details = vec![
                field("codec").to_owned(),
                field("lang").to_owned(),
                field("title").to_owned(),
            ];
            if kind == "audio" {
                if let Some(n) = t["demux-samplerate"].as_u64() {
                    details.push(format!("{n} Hz"));
                }
                if let Some(n) = t["demux-channel-count"].as_u64() {
                    details.push(format!("{n} canales"));
                }
                if let Some(n) = t["demux-bitrate"].as_u64() {
                    details.push(cineplayer_core::media::bitrate(Some(n.to_string())));
                }
            }
            details.retain(|s| !s.is_empty());
            lines.push(format!("{selected} {id} · {}", details.join(" · ")));
        }
        if !found {
            lines.push("Sin pistas".into());
        }
        if kind == "audio" {
            lines.push(format!(
                "Salida: {} · {}",
                get("audio-out-params/format"),
                get("audio-out-params/hr-channels")
            ));
        }
    }
    let entries = lines
        .into_iter()
        .map(|label| {
            let mut e = popup::Entry::action(0, label);
            e.enabled = false;
            e
        })
        .collect();
    let _ = popup::show_information(m, "Información de audio y vídeo", entries);
}
fn action(m: &Mpv, s: &mut State, a: Action) -> Result<(), String> {
    if s.closing {
        return Ok(());
    }
    match a {
        Action::Track(k, v) => return m.command_async(&["set", &k, &v], 3),
        Action::Device(d) => {
            m.set("audio-device", &d)?;
            s.device = if d == "auto" { String::new() } else { d };
            return Ok(());
        }
        Action::Id(id) => match id {
            1 => return open(m, s, false),
            2 => return open(m, s, true),
            20..=25 => {
                s.prefs.aspect = match id {
                    21 => Some(16.0 / 9.0),
                    22 => Some(4.0 / 3.0),
                    23 => Some(1.85),
                    24 => Some(2.35),
                    25 => Some(2.39),
                    _ => None,
                }
            }
            30..=31 => {
                let sdr = id == 31;
                s.hdr_sdr = sdr;
                s.applied = None;
                apply(m, s)?;
                return text(
                    m,
                    if sdr {
                        "Salida SDR"
                    } else {
                        "Color automático: revisa HDR en Windows"
                    },
                );
            }
            40..=42 => {
                let mode = (id - 40) as u8;
                let old = m.get("audio-spdif").unwrap_or_default();
                let old_ex = m.get("audio-exclusive").unwrap_or_else(|| "no".into());
                let result = (|| {
                    m.set("audio-exclusive", if mode == 0 { "no" } else { "yes" })?;
                    m.set(
                        "audio-spdif",
                        match mode {
                            1 => "ac3,dts",
                            2 => "ac3,eac3,dts,dts-hd,truehd",
                            _ => "",
                        },
                    )
                })();
                if let Err(e) = result {
                    let _ = m.set("audio-spdif", &old);
                    let _ = m.set("audio-exclusive", &old_ex);
                    return Err(e);
                }
                s.passthrough = mode;
                return text(
                    m,
                    if mode == 0 {
                        "Audio decodificado a PCM"
                    } else {
                        "Passthrough: requiere HDMI/SPDIF y receptor compatibles"
                    },
                );
            }
            50 => return m.command_async(&["cycle", "pause"], 2),
            51 => return m.command_async(&["cycle", "fullscreen"], 2),
            52 => return original(m),
            53 => {
                info(m, s);
                return Ok(());
            }
            54 => return help(m),
            56 => {
                windows::player_message(
                    m,
                    &format!(
                        "{APP_NAME_VERSION}\nReproductor de vídeo para Windows 11 (x64)\n\nMotor de reproducción: {}\nFFmpeg: {}",
                        m.get("mpv-version")
                            .unwrap_or_else(|| "No disponible".into()),
                        m.get("ffmpeg-version")
                            .unwrap_or_else(|| "No disponible".into())
                    ),
                    false,
                );
                return Ok(());
            }
            57 => {
                updater::start_manual();
                return Ok(());
            }
            58 => {
                let delay = m
                    .get("sub-delay")
                    .and_then(|v| v.parse::<f64>().ok())
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0);
                let chosen = windows::choose(
                    m,
                    "Sincronización de subtítulos",
                    &format!(
                        "Retraso actual: {delay:+.2} s\nUn valor positivo retrasa los subtítulos."
                    ),
                    &[
                        (101, "Adelantar 0,5 s"),
                        (102, "Adelantar 0,1 s"),
                        (103, "Restablecer a 0"),
                        (104, "Retrasar 0,1 s"),
                        (105, "Retrasar 0,5 s"),
                    ],
                )?;
                let next = match chosen {
                    101 => delay - 0.5,
                    102 => delay - 0.1,
                    103 => 0.0,
                    104 => delay + 0.1,
                    105 => delay + 0.5,
                    _ => return Ok(()),
                };
                return m.command_async(
                    &[
                        "set",
                        "sub-delay",
                        &next.clamp(-86400.0, 86400.0).to_string(),
                    ],
                    3,
                );
            }
            55 => {
                s.capture_position(m);
                s.flush_history();
                m.reset_seeks();
                s.closing = true;
                return m.command_async(&["quit"], 0);
            }
            1000..=6095 => return player::action(m, s, id),
            _ => return Ok(()),
        },
    }
    s.applied = None;
    apply(m, s)?;
    text(m, s.prefs.output.label())
}

fn run() -> Result<(), String> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let probe = args.first().is_some_and(|v| v == "--probe");
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("Ruta de aplicación desconocida")?;
    // Declared before m so its HICONs outlive the mpv window on every exit path.
    let mut branding = windows::Branding::new()?;
    let mut controls = controls::Controls::default();
    let mut m = Mpv::new(&dir.join("libmpv-2.dll"))?;
    configure(&m, probe, dir)?;
    m.init()?;
    for (i, p) in [
        "sid",
        "sub-ass-extradata",
        "sub-text/ass-full",
        "metadata",
        "video-params/w",
        "aid",
        "track-list",
    ]
    .iter()
    .enumerate()
    {
        m.observe_string(p, 100 + i as u64)?;
    }
    m.register_mvc(Box::new(mvc::Registry::new(
        &dir.join("runtime/cine_mvc.dll"),
    )))?;
    bind(&m)?;
    let startup_probe = if probe {
        std::env::var_os("ENIGMAX_PROBE_STARTUP_CONFIG").map(PathBuf::from)
    } else {
        None
    };
    let mut state = if let Some(config) = &startup_probe {
        State::new(config, false)
    } else {
        State::new(dir, probe)
    };
    if !probe {
        updater::start();
    }
    let mut output = None;
    let file = if probe {
        if args.len() != 4 {
            return Err(
                "Uso: EnigmaxCinePlayer --probe archivo original|fsbs|hsbs|hou|mvc[-ana][-gm|-ab|-rb|-rg][-cpu] captura.png".into(),
            );
        }
        let probe_mode = args[2].to_str().ok_or("Modo de prueba inválido")?;
        let parts = probe_mode.split('-').collect::<Vec<_>>();
        state.prefs.layout = if parts[0] == "original" {
            Layout::Mono
        } else {
            Layout::parse(parts[0]).ok_or("Formato de prueba inválido")?
        };
        state.prefs.output = if parts.contains(&"ana") {
            Output::Anaglyph
        } else if parts.contains(&"right") {
            Output::Right
        } else if parts[0] == "original"
            || parts.contains(&"both")
            || parts.contains(&"sbs1080")
            || parts.contains(&"sbs2160")
            || parts.contains(&"framepack")
        {
            Output::Original
        } else {
            Output::Left
        };
        state.prefs.delivery = if parts.contains(&"framepack") {
            cineplayer_core::stereo::Delivery::FramePack
        } else if parts.contains(&"sbs1080") {
            cineplayer_core::stereo::Delivery::FullSbs
        } else if parts.contains(&"sbs2160") {
            cineplayer_core::stereo::Delivery::Anamorphic
        } else {
            cineplayer_core::stereo::Delivery::Native
        };
        state.prefs.glasses = if parts.contains(&"gm") {
            Glasses::GreenMagenta
        } else if parts.contains(&"ab") {
            Glasses::AmberBlue
        } else if parts.contains(&"rb") {
            Glasses::RedBlue
        } else if parts.contains(&"rg") {
            Glasses::RedGreen
        } else {
            Glasses::RedCyan
        };
        state.prefs.algorithm = if parts.contains(&"color") {
            Algorithm::Color
        } else if parts.contains(&"mono") {
            Algorithm::Mono
        } else if parts.contains(&"half") {
            Algorithm::Half
        } else {
            Algorithm::Dubois
        };
        if parts.contains(&"suboff") {
            state.prefs.ass_mode = cineplayer_core::ass3d::Mode::Off;
        }
        state.prefs.swap = parts.contains(&"swap");
        if parts.contains(&"cal") {
            state.prefs.left_gain = 1.1;
            state.prefs.right_gain = 0.9;
        }
        if parts.contains(&"subforce") {
            state.prefs.ass_mode = if state.prefs.layout.horizontal() {
                cineplayer_core::ass3d::Mode::Sbs
            } else {
                cineplayer_core::ass3d::Mode::Ou
            };
        }
        state.prefs.quality = if parts.contains(&"low") {
            cineplayer_core::stereo::Quality::Low
        } else if parts.contains(&"high") {
            cineplayer_core::stereo::Quality::High
        } else if parts.contains(&"cpu") {
            cineplayer_core::stereo::Quality::Compatibility
        } else {
            cineplayer_core::stereo::Quality::Balanced
        };
        state.hdr_sdr = parts.contains(&"sdr");
        output = Some(PathBuf::from(&args[3]));
        Some(&args[1])
    } else {
        if args.len() > 1 {
            return Err("Abre un único vídeo cada vez".into());
        }
        args.first()
    };
    if startup_probe.is_some() {
        state.begin_startup(&m)?;
    } else if let Some(file) = file {
        let path = std::path::absolute(file).map_err(|e| e.to_string())?;
        if !path.is_file() {
            return Err(format!("No existe el archivo: {}", path.display()));
        }
        let path = path.to_str().ok_or("Ruta Unicode inválida")?;
        if probe && std::env::var_os("ENIGMAX_PROBE_DIRECT").is_some() {
            m.command(&["loadfile", path, "replace"])?;
        } else {
            player::load(&m, &mut state, path, None)?;
        }
    } else if !probe && let Err(error) = state.begin_startup(&m) {
        let _ = state.cancel_startup(&m);
        let _ = m.set("pause", "no");
        let _ = std::fs::write(dir.join("config/startup-error.txt"), error);
    }
    let started = Instant::now();
    let play_seconds = std::env::var("ENIGMAX_PROBE_PLAY_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2)
        .clamp(2, 120);
    let trace = std::env::var_os("ENIGMAX_CINEPLAYER_TRACE").is_some();
    let mut ready = None;
    let mut redirecting = false;
    let mut probe_sub_added = false;
    let mut sync_probe = if probe && std::env::var_os("ENIGMAX_PROBE_SYNC").is_some() {
        Some(sync_probe::SyncProbe::new(
            output.as_ref().ok_or("Falta ruta de informe")?,
        )?)
    } else {
        None
    };
    loop {
        // Only a pending bounded background probe needs timed wakeups. Normal
        // playback sleeps until libmpv delivers an event; no resident polling.
        let e = m.wait(if probe {
            0.2
        } else if state.detection.as_ref().is_some_and(|p| p.waiting()) {
            0.1
        } else {
            -1.0
        });
        let id = e.id;
        let reply = (e.userdata, e.error);
        // Do not issue more commands after quit. END_FILE/QUIT can precede
        // SHUTDOWN while older control messages are still queued.
        if id == 1
            || (id == 7 && !e.data.is_null() && unsafe { (*e.data.cast::<EndFile>()).reason } == 3)
        {
            state.flush_history();
            m.reset_seeks();
            break;
        }
        if trace {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("diagnostico-eventos.txt"))
            {
                let _ = writeln!(f, "event {id}");
            }
        }
        let client_args = if id == 16 && !e.data.is_null() {
            let data = unsafe { &*e.data.cast::<ClientMessage>() };
            if data.count > 0 && data.count <= 16 && !data.args.is_null() {
                unsafe { std::slice::from_raw_parts(data.args, data.count as usize) }
                    .iter()
                    .filter_map(|p| {
                        if p.is_null() {
                            None
                        } else {
                            let value = unsafe { CStr::from_ptr(*p) };
                            (value.to_bytes().len() <= 8192)
                                .then(|| value.to_string_lossy().into_owned())
                        }
                    })
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        let client = client_args.first().cloned();
        let end_error = if id == 7 && !e.data.is_null() {
            let data = unsafe { &*e.data.cast::<EndFile>() };
            if data.reason == 4 {
                Some(data.error)
            } else {
                None
            }
        } else {
            None
        };
        if matches!(id, 6 | 8 | 16 | 17 | 22) && m.get("idle-active").is_none() {
            // The core can already be shutting down before its queued SHUTDOWN
            // reaches this client. Discard stale UI work in that interval.
            continue;
        }
        if id == 6 {
            m.reset_seeks();
            if let Err(e) = controls.hide(&m) {
                if probe {
                    return Err(e);
                }
                runtime_error(&m, dir, "Controles", &e);
            }
            if !state.pending_load {
                state.cancel_startup(&m)?;
                state.new_open();
            }
            state.pending_load = true;
            redirecting = false;
            let incoming = m
                .get("playlist-pos")
                .and_then(|n| m.get(&format!("playlist/{n}/filename")))
                .or_else(|| m.get("path"));
            if let Some(path) = incoming
                && !path.starts_with("bd://")
                && !path.starts_with("cine-mvc://")
                && !path.starts_with("cine-disc://")
                && path != state.source
            {
                // Drag and drop reaches mpv as loadfile. Redirect at START_FILE,
                // before the ISO can fail the ordinary media demuxer.
                match player::load(&m, &mut state, &path, None) {
                    Ok(()) => {
                        redirecting = true;
                        continue;
                    }
                    Err(e) => {
                        if probe {
                            return Err(e);
                        }
                        runtime_error(&m, dir, "Apertura del archivo", &e);
                    }
                }
            }
        }
        let mut disc_detail = String::new();
        if end_error.is_some()
            && !redirecting
            && !state.disc_direct
            && state.raw.is_none()
            && !state.titles.is_empty()
        {
            match player::fallback_disc(&m, &mut state) {
                Ok(true) => continue,
                Err(detail) => {
                    let _ = std::fs::write(dir.join("config/disc-fallback-error.txt"), &detail);
                    disc_detail = detail;
                }
                _ => {}
            }
        }
        if let Some(err) = end_error
            && !redirecting
        {
            let detail = if disc_detail.is_empty() {
                m.mvc()?.error()
            } else {
                disc_detail
            };
            let msg = format!("No se pudo reproducir el archivo (libmpv {err}). {detail}");
            if probe {
                return Err(msg);
            } else if state.startup_choice {
                state.cancel_startup(&m)?;
                let _ = std::fs::write(dir.join("config/startup-error.txt"), &msg);
            } else {
                runtime_error(&m, dir, "Archivo", &msg);
            }
        }
        if id == 1 {
            break;
        }
        if id == 7 {
            m.reset_seeks();
            state.pending_load = true;
            let _ = controls.hide(&m);
        }
        if id == 5 {
            if reply.0 & (1_u64 << 63) != 0
                && let Err(e) = m.seek_reply(reply.0)
            {
                if probe {
                    return Err(e);
                }
                runtime_error(&m, dir, "Búsqueda", &e);
            }
            if reply.1 < 0 && reply.0 != 0 && !state.pending_load {
                if probe {
                    return Err(format!("Comando asíncrono {}: libmpv {}", reply.0, reply.1));
                }
                runtime_error(
                    &m,
                    dir,
                    if reply.0 == 3 {
                        "Cambio de pista"
                    } else {
                        "Control de reproducción"
                    },
                    &format!(
                        "El motor no pudo completar la operación (libmpv {}). Puede haber terminado la reproducción o cambiado el archivo.",
                        reply.1
                    ),
                );
            }
        }
        if id == 8 {
            state.pending_load = false;
            if let Some(path) = m.get("path")
                && !path.starts_with("bd://")
                && !path.starts_with("cine-mvc://")
                && !path.starts_with("cine-disc://")
                && path != state.source
            {
                player::load(&m, &mut state, &path, None)?;
                continue;
            }
            state.applied = None;
            if probe
                && !probe_sub_added
                && let Ok(file) = std::env::var("ENIGMAX_PROBE_SUB_FILE")
            {
                m.command(&["sub-add", &file, "select"])?;
                probe_sub_added = true;
            }
            if probe && let Ok(sid) = std::env::var("ENIGMAX_PROBE_SID") {
                m.set("sid", &sid)?;
            }
            if probe && state.resume.is_none() {
                state.resume = std::env::var("ENIGMAX_PROBE_START")
                    .ok()
                    .filter(|v| !v.is_empty());
            }
        }
        state.restore_tracks(&m)?;
        if id == 8 || id == 17 || (id == 22 && matches!(reply.0, 100 | 105 | 106)) {
            state.capture_tracks(&m);
        }
        let subs_changed = if !state.pending_load && (id == 22 || id == 8) {
            match player::subtitle_event(&m, &mut state) {
                Ok(changed) => changed,
                Err(e) => {
                    if probe {
                        return Err(e);
                    }
                    runtime_error(&m, dir, "ASS 3D", &e);
                    state.prefs.ass_mode = cineplayer_core::ass3d::Mode::Off;
                    state.applied = None;
                    true
                }
            }
        } else {
            false
        };
        if id == 8 || id == 17 || subs_changed {
            if let Err(e) = branding.apply(owner(&m)) {
                if probe {
                    return Err(e);
                }
                runtime_error(&m, dir, "Icono de ventana", &e);
            }
            match apply(&m, &mut state) {
                Ok(()) => {
                    if state.applied.is_some() && ready.is_none() {
                        ready = Some(Instant::now());
                    }
                }
                Err(e) => {
                    if probe {
                        return Err(e);
                    }
                    runtime_error(&m, dir, "Presentación de vídeo", &e);
                    state.prefs.layout = Layout::Mono;
                    state.prefs.output = Output::Original;
                    state.applied = None;
                    apply(&m, &mut state)?;
                }
            }
        }
        if let Some(cmd) = client {
            if trace {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join("diagnostico-eventos.txt"))
                {
                    let _ = writeln!(f, "client {cmd}; window {}", owner(&m));
                }
            }
            let result = match cmd.as_str() {
                "controls-ready" => {
                    controls.ready();
                    if probe && std::env::var_os("ENIGMAX_PROBE_CONTROLS").is_some() {
                        m.command(&["script-message-to", "enigmax_controls", "pin", "yes"])?;
                    }
                    Ok(())
                }
                "controls-hide" => controls.hide(&m),
                "controls-draw" | "controls-input"
                    if state.pending_load || m.get("idle-active").as_deref() != Some("no") =>
                {
                    Ok(())
                }
                "controls-draw" => client_args
                    .get(1)
                    .ok_or_else(|| "Faltan datos de controles".to_owned())
                    .and_then(|body| controls.draw(&m, body)),
                "controls-input" => {
                    if let (Some(kind), Some(body)) = (client_args.get(1), client_args.get(2)) {
                        controls
                            .input(&m, kind, body)
                            .and_then(|input| match input {
                                controls::Input::None => Ok(()),
                                controls::Input::Info => {
                                    info(&m, &state);
                                    Ok(())
                                }
                                controls::Input::Swap => player::action(&m, &mut state, 1024),
                                controls::Input::Audio => choose_track(&m, &mut state, false),
                                controls::Input::Subtitles => choose_track(&m, &mut state, true),
                                controls::Input::Options => menu(&m, &state, true).and_then(|a| {
                                    if let Some(a) = a {
                                        action(&m, &mut state, a)
                                    } else {
                                        Ok(())
                                    }
                                }),
                                controls::Input::Chapters => chapter_navigation(&m, None),
                                controls::Input::ChapterStep(step) => {
                                    chapter_navigation(&m, Some(step))
                                }
                                controls::Input::Menu => player::choose_view(&m, &mut state),
                            })
                    } else {
                        Err("Faltan datos del ratón".into())
                    }
                }
                "stereo-menu" => player::stereo_menu(&m, &state)
                    .and_then(|menu| menu.show(&m))
                    .and_then(|id| {
                        if id == 0 {
                            Ok(())
                        } else {
                            action(&m, &mut state, Action::Id(id))
                        }
                    }),
                "menu" => menu(&m, &state, false).and_then(|a| {
                    if let Some(a) = a {
                        action(&m, &mut state, a)
                    } else {
                        Ok(())
                    }
                }),
                "toggle-pause" => {
                    if !state.pending_load && m.get("idle-active").as_deref() == Some("no") {
                        m.command_async(&["cycle", "pause"], 1)
                    } else {
                        Ok(())
                    }
                }
                "quit" => action(&m, &mut state, Action::Id(55)),
                "checkpoint" => {
                    if client_args.get(3) == m.get("path").as_ref()
                        && let (Some(pos), Some(duration)) =
                            (client_args.get(1), client_args.get(2))
                    {
                        state.checkpoint(pos, duration);
                    }
                    Ok(())
                }
                "open" => open(&m, &mut state, false),
                "idle-draw" => controls.idle_draw(&m),
                "idle-click" => Ok(()),
                "startup-draw" => controls.startup_draw(&m, &state),
                "startup-choice" => finish_startup(
                    &m,
                    &mut state,
                    client_args.get(1).is_some_and(|v| v == "restart"),
                ),
                "startup-click" => {
                    let point = client_args
                        .get(1)
                        .and_then(|x| x.parse::<i32>().ok())
                        .zip(client_args.get(2).and_then(|y| y.parse::<i32>().ok()));
                    if let Some(restart) = point.and_then(|(x, y)| controls.startup_hit(&m, x, y)) {
                        finish_startup(&m, &mut state, restart)
                    } else {
                        Ok(())
                    }
                }
                "sub" => open(&m, &mut state, true),
                "original" => original(&m),
                "help" => help(&m),
                _ => Ok(()),
            };
            if let Err(e) = result {
                if state.closing {
                    break;
                }
                if probe {
                    return Err(e);
                }
                runtime_error(&m, dir, "Acción del usuario", &e);
            }
        }
        // Do not process queued seek/track/UI replies once the user requested
        // quit. mpv_terminate_destroy performs the complete native teardown.
        if state.closing {
            break;
        }
        if id == 8 || id == 17 || subs_changed || (id == 22 && reply.0 == 103) {
            if !state.pending_load {
                state.display_title = cineplayer_core::media::title(
                    &state.source,
                    &m.get("metadata").unwrap_or_default(),
                );
            }
            player::ask_resume(&m, &mut state)?;
            player::ask_view(&m, &mut state)?;
        }
        state.restore_position(&m)?;
        if let Err(error) = state.remember_ready(&m) {
            let _ = std::fs::write(dir.join("config/last-video-error.txt"), error);
        }
        if let Err(error) = player::poll_detection(&m, &mut state) {
            if probe {
                return Err(error);
            }
            runtime_error(&m, dir, "Detección estructural 3D", &error);
        }
        if let Err(e) = controls.sync(&m, &state) {
            if probe {
                return Err(e);
            }
            runtime_error(&m, dir, "Controles", &e);
        }
        if let Some(sync) = &mut sync_probe {
            sync.tick(&m, &mut state)?;
        }
        if probe {
            if started.elapsed() > Duration::from_secs(play_seconds + 30) {
                return Err("Tiempo agotado al probar la reproducción".into());
            }
            if ready.is_some_and(|t| t.elapsed() > Duration::from_secs(play_seconds)) {
                let ui = std::env::var_os("ENIGMAX_PROBE_CONTROLS").is_some();
                if ui && !controls.rendered() {
                    return Err("No se han renderizado los controles inferiores".into());
                }
                let path = output.as_ref().ok_or("Falta captura")?;
                m.command(&[
                    "screenshot-to-file",
                    path.to_str().ok_or("Ruta de captura inválida")?,
                    if ui || startup_probe.is_some() {
                        "window"
                    } else {
                        "subtitles"
                    },
                ])?;
                let report = [
                    "title",
                    "pause",
                    "user-data/enigmax-startup",
                    "scale",
                    "dscale",
                    "hdr-compute-peak",
                    "demuxer-max-bytes",
                    "demuxer-max-back-bytes",
                    "current-ao",
                    "sub-delay",
                    "sub-start",
                    "sub-end",
                    "glsl-shaders",
                    "current-vo",
                    "blend-subtitles",
                    "sub-ass-video-aspect-override",
                    "vf",
                    "vo-passes",
                    "time-pos",
                    "duration",
                    "audio-delay",
                    "avsync",
                    "total-avsync-change",
                    "frame-drop-count",
                    "decoder-frame-drop-count",
                    "audio-pts",
                    "estimated-vf-fps",
                    "demuxer-cache-state",
                    "mpv-version",
                    "video-crop",
                    "video-aspect-override",
                    "video-params/w",
                    "video-params/h",
                    "video-out-params/dw",
                    "video-out-params/dh",
                    "video-out-params/stereo-in",
                    "osd-width",
                    "osd-height",
                    "hwdec-current",
                    "hwdec",
                    "hwdec-software-fallback",
                    "vd-lavc-threads",
                    "display-hidpi-scale",
                    "video-params/primaries",
                    "video-params/gamma",
                    "target-trc",
                    "target-prim",
                    "sid",
                    "sub-text",
                    "audio-codec-name",
                    "audio-params",
                    "audio-out-params",
                    "audio-device",
                    "audio-spdif",
                    "audio-exclusive",
                    "audio-channels",
                    "track-list",
                    "chapter-list",
                ]
                .iter()
                .map(|p| format!("{p}: {}", m.get(p).unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("\n");
                std::fs::write(
                    path.with_extension("txt"),
                    format!(
                        "{}\nMPLS: {}\nMVC: {:?}\nPipeline: {}\nError MVC: {}\nASS 3D: {}",
                        format_args!(
                            "{report}\nVentana / monitor: {}",
                            windows::display_geometry(owner(&m)).unwrap_or_default()
                        ),
                        state.title,
                        state.raw,
                        state.pipeline,
                        m.mvc()?.error(),
                        state
                            .subtitles
                            .status(state.prefs.ass_mode, state.effective(&m))
                    ),
                )
                .map_err(|e| e.to_string())?;
                break;
            }
        }
    }
    state.flush_history();
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        let is_probe = std::env::args_os().nth(1).is_some_and(|v| v == "--probe");
        if !is_probe {
            windows::message(&e, true);
        }
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::write(exe.with_file_name("ultimo-error.txt"), &e);
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cineplayer_core::stereo::Settings;
    #[test]
    fn quit_enters_terminal_state_before_pending_control_replies() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut state = State::new(temp.path(), true);
        m.command_async(&["set", "invalid-property-for-shutdown-test", "x"], 3)
            .unwrap();
        let start = Instant::now();
        action(&m, &mut state, Action::Id(55)).unwrap();
        assert!(state.closing);
        // A queued action must not change preferences or submit more commands.
        action(&m, &mut state, Action::Id(31)).unwrap();
        assert!(!state.hdr_sdr);
        drop(m);
        assert!(start.elapsed() < Duration::from_secs(10));
    }
    #[test]
    fn chapter_navigation_reads_real_metadata_and_clamps_boundaries() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("pause", "yes"),
            ("idle", "yes"),
        ] {
            m.option(k, v).unwrap();
        }
        m.init().unwrap();
        m.command(&[
            "loadfile",
            fixture_dir().join("chapters-qa.mkv").to_str().unwrap(),
        ])
        .unwrap();
        let until = Instant::now() + Duration::from_secs(8);
        while chapter_entries(&m).len() != 3 {
            m.wait(0.02);
            assert!(Instant::now() < until, "No se cargan los capítulos");
        }
        let entries = chapter_entries(&m);
        assert_eq!(entries[1].1, 8.0);
        assert_eq!(entries[1].2, "Segundo capítulo");
        for (step, expected) in [(-1, 0), (1, 1), (1, 2), (1, 2), (-1, 1)] {
            chapter_navigation(&m, Some(step)).unwrap();
            let until = Instant::now() + Duration::from_secs(3);
            while m.get("chapter").as_deref() != Some(&expected.to_string()) {
                m.wait(0.02);
                assert!(Instant::now() < until, "No se llega al capítulo {expected}");
            }
        }
    }
    fn fixture_dir() -> PathBuf {
        std::env::var_os("ENIGMAX_TEST_FIXTURES")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verificacion/muestras")
            })
    }
    #[test]
    fn obsolete_depth_profiles_cannot_shift_real_video() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut s = State::new(temp.path(), true);
        for (name, layout) in [
            ("fullsbs.mkv", Layout::FullSbs),
            ("hsbs.mkv", Layout::HalfSbs),
            ("fullou.mkv", Layout::FullOu),
            ("hou.mkv", Layout::HalfOu),
        ] {
            s.prefs.layout = layout;
            s.prefs.output = Output::Original;
            player::load(
                &m,
                &mut s,
                root.join("verificacion/muestras")
                    .join(name)
                    .to_str()
                    .unwrap(),
                None,
            )
            .unwrap();
            pump(&mut m, &mut s, |m, s| {
                !s.pending_load && m.get("time-pos").is_some()
            });
            m.set("sid", "no").unwrap();
            for (output, delivery) in [
                (Output::Original, cineplayer_core::stereo::Delivery::Native),
                (
                    Output::Original,
                    cineplayer_core::stereo::Delivery::Anamorphic,
                ),
                (
                    Output::Original,
                    cineplayer_core::stereo::Delivery::FramePack,
                ),
                (Output::Anaglyph, cineplayer_core::stereo::Delivery::Native),
            ] {
                s.prefs.output = output;
                s.prefs.delivery = delivery;
                s.prefs = Settings::decode(&(s.prefs.encode() + "convergence=-0.02\n")).unwrap();
                s.applied = None;
                apply(&m, &mut s).unwrap();
                settle(&mut m, &mut s);
                let before = m.screenshot_pixels().unwrap();
                s.prefs = Settings::decode(&(s.prefs.encode() + "convergence=0.02\n")).unwrap();
                s.applied = None;
                apply(&m, &mut s).unwrap();
                settle(&mut m, &mut s);
                let after = m.screenshot_pixels().unwrap();
                assert!(
                    changed_pixels(&before, &after) <= 30,
                    "Obsolete profile changed {name} {output:?} {delivery:?}"
                );
            }
        }
    }
    #[test]
    fn obsolete_depth_profiles_preserve_subtitles_and_authored_ass() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option(
            "log-file",
            root.join("verificacion/subtitle-depth.log")
                .to_str()
                .unwrap(),
        )
        .unwrap();
        m.option("msg-level", "all=v").unwrap();
        m.init().unwrap();
        for (i, p) in ["sid", "sub-ass-extradata", "sub-text/ass-full"]
            .iter()
            .enumerate()
        {
            m.observe_string(p, 100 + i as u64).unwrap();
        }
        let mut s = State::new(temp.path(), true);
        s.prefs.layout = Layout::HalfSbs;
        s.prefs.output = Output::Original;
        player::load(
            &m,
            &mut s,
            root.join("verificacion/muestras/ass3d-black.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        pump(&mut m, &mut s, |m, s| {
            !s.pending_load && m.get("time-pos").is_some()
        });
        m.seek(1.0, true).unwrap();
        settle(&mut m, &mut s);
        fn centers(shot: &(usize, usize, Vec<u8>), horizontal: bool) -> [f64; 2] {
            let (w, h, pixels) = shot;
            let mut sums = [0_u64; 2];
            let mut counts = [0_u64; 2];
            for y in 0..*h {
                for x in 0..*w {
                    let i = (y * w + x) * 4;
                    if pixels[i..i + 3].iter().all(|v| *v > 200) {
                        let eye = if horizontal {
                            usize::from(x >= w / 2)
                        } else {
                            usize::from(y >= h / 2)
                        };
                        sums[eye] += x as u64;
                        counts[eye] += 1;
                    }
                }
            }
            assert!(
                counts.iter().all(|n| *n > 20),
                "Missing subtitle eye: {counts:?}"
            );
            [
                sums[0] as f64 / counts[0] as f64,
                sums[1] as f64 / counts[1] as f64,
            ]
        }
        for extension in ["srt", "ass", "sup"] {
            if extension == "sup" {
                player::load(
                    &m,
                    &mut s,
                    root.join("verificacion/muestras/depth-qa-pgs.mkv")
                        .to_str()
                        .unwrap(),
                    None,
                )
                .unwrap();
                pump(&mut m, &mut s, |m, s| {
                    !s.pending_load && m.get("time-pos").is_some()
                });
                m.set("sid", "1").unwrap();
            } else {
                m.command(&[
                    "sub-add",
                    root.join(format!("verificacion/muestras/depth-qa.{extension}"))
                        .to_str()
                        .unwrap(),
                    "select",
                ])
                .unwrap();
            }
            settle(&mut m, &mut s);
            m.seek(0.0, true).unwrap();
            m.set("pause", "no").unwrap();
            settle(&mut m, &mut s);
            m.set("pause", "yes").unwrap();
            for layout in [
                Layout::FullSbs,
                Layout::HalfSbs,
                Layout::FullOu,
                Layout::HalfOu,
            ] {
                s.prefs.layout = layout;
                s.prefs.ass_mode = cineplayer_core::ass3d::Mode::Off;
                let mut measurements = vec![];
                for depth in [-0.02, 0.0, 0.02] {
                    s.prefs = Settings::decode(&format!("{}sub_depth={depth}\n", s.prefs.encode()))
                        .unwrap();
                    s.applied = None;
                    apply(&m, &mut s).unwrap();
                    settle(&mut m, &mut s);
                    m.command(&[
                        "screenshot-to-file",
                        root.join(format!(
                            "verificacion/depth-{extension}-{layout:?}-{depth:.2}.png"
                        ))
                        .to_str()
                        .unwrap(),
                        "subtitles",
                    ])
                    .unwrap();
                    measurements.push(centers(
                        &m.screenshot_pixels().unwrap(),
                        layout.horizontal(),
                    ));
                }
                assert!(
                    (measurements[2][0] - measurements[0][0]).abs() < 0.5,
                    "Left {extension} {layout:?}: {measurements:?}"
                );
                assert!(
                    (measurements[2][1] - measurements[0][1]).abs() < 0.5,
                    "Right {extension} {layout:?}: {measurements:?}"
                );
                println!("{extension} {layout:?}: measured subtitle centers {measurements:?}");
            }
        }
        // Restore the embedded stereoscopic ASS. Its original disparity remains;
        // obsolete profile depth must not shift either authored eye position.
        m.command(&[
            "sub-add",
            root.join("verificacion/muestras/ass3d-sbs.ass")
                .to_str()
                .unwrap(),
            "select",
        ])
        .unwrap();
        s.prefs.layout = Layout::HalfSbs;
        s.prefs.ass_mode = cineplayer_core::ass3d::Mode::Sbs;
        settle(&mut m, &mut s);
        let mut measurements = vec![];
        for depth in [-0.02, 0.02] {
            s.prefs =
                Settings::decode(&format!("{}sub_depth={depth}\n", s.prefs.encode())).unwrap();
            s.applied = None;
            apply(&m, &mut s).unwrap();
            settle(&mut m, &mut s);
            measurements.push(centers(&m.screenshot_pixels().unwrap(), true));
        }
        assert!(
            (measurements[1][0] - measurements[0][0]).abs() < 0.5,
            "Packed ASS left {measurements:?}"
        );
        assert!(
            (measurements[1][1] - measurements[0][1]).abs() < 0.5,
            "Packed ASS right {measurements:?}"
        );
    }
    fn settle(m: &mut Mpv, s: &mut State) {
        let until = Instant::now() + Duration::from_millis(400);
        pump(m, s, |_, _| Instant::now() >= until);
    }
    fn changed_pixels(a: &(usize, usize, Vec<u8>), b: &(usize, usize, Vec<u8>)) -> usize {
        let w = a.0.min(b.0).min(960);
        let h = a.1.min(b.1).min(540);
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let ai = ((y * a.1 / h) * a.0 + x * a.0 / w) * 4;
                let bi = ((y * b.1 / h) * b.0 + x * b.0 / w) * 4;
                a.2[ai..ai + 3]
                    .iter()
                    .zip(&b.2[bi..bi + 3])
                    .any(|(a, b)| a.abs_diff(*b) > 32)
            })
            .count()
    }
    #[test]
    #[ignore = "Requires ENIGMAX_TEST_CLEAR_DISC_ISO pointing to the authorized clear ISO"]
    fn clear_iso_fallback_seeks_tracks_and_quit() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = std::env::var("ENIGMAX_TEST_CLEAR_DISC_ISO").unwrap();
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("pause", "yes").unwrap();
        if let Some(log) = std::env::var_os("ENIGMAX_TEST_ISO_LOG") {
            m.option("log-file", log.to_str().unwrap()).unwrap();
            m.option("msg-level", "all=debug").unwrap();
        }
        m.init().unwrap();
        m.register_mvc(Box::new(mvc::Registry::new(
            &root.join("runtime/cine_mvc.dll"),
        )))
        .unwrap();
        let mut state = State::new(temp.path(), true);
        player::load(&m, &mut state, &path, None).unwrap();
        let until = Instant::now() + Duration::from_secs(15);
        loop {
            let e = m.wait(0.05);
            if e.id == 7 && !e.data.is_null() && unsafe { (*e.data.cast::<EndFile>()).reason } == 4
            {
                break;
            }
            assert!(Instant::now() < until);
        }
        assert!(player::fallback_disc(&m, &mut state).unwrap());
        pump(&mut m, &mut state, |m, s| {
            !s.pending_load && m.get("video-params/w").as_deref() == Some("3840")
        });
        assert!(
            !player::fallback_disc(&m, &mut state).unwrap(),
            "No retry loops"
        );
        for target in [120.0, 600.0, 8000.0, 30.0] {
            m.seek(target, true).unwrap();
            pump(&mut m, &mut state, |m, _| {
                m.get("time-pos")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| (v - target).abs() < 0.15)
            });
            println!("ISO seek {target}: {:?}", m.get("time-pos"));
        }
        let (_, _, tracks) = track_menus(&m).unwrap();
        let mut audio = 0;
        let mut sub = 0;
        for (_, track) in tracks {
            if let Action::Track(key, value) = track {
                if key == "aid" && audio < 2 {
                    action(&m, &mut state, Action::Track(key.clone(), value.clone())).unwrap();
                    pump(&mut m, &mut state, |m, _| {
                        m.get(&key).as_deref() == Some(value.as_str())
                    });
                    audio += 1;
                }
                if key == "sid" && value != "no" && sub == 0 {
                    action(&m, &mut state, Action::Track(key.clone(), value.clone())).unwrap();
                    pump(&mut m, &mut state, |m, _| {
                        m.get(&key).as_deref() == Some(value.as_str())
                    });
                    sub += 1;
                }
            }
        }
        assert_eq!((audio, sub), (2, 1));
        action(&m, &mut state, Action::Id(55)).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            let event = m.wait(0.05);
            if event.id != 0 {
                println!("ISO quit event={} error={}", event.id, event.error);
            }
            if event.id == 1 {
                break;
            }
            assert!(Instant::now() < until);
        }
    }
    #[test]
    fn clear_disc_stream_is_verified_and_malformed_or_multiclip_is_rejected() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        for dir in ["AACS", "BDMV/PLAYLIST", "BDMV/STREAM"] {
            std::fs::create_dir_all(temp.path().join(dir)).unwrap();
        }
        std::fs::write(
            temp.path().join("AACS/Unit_Key_RO.inf"),
            b"synthetic metadata only",
        )
        .unwrap();
        let playlist = |count: u16| {
            let mut b = vec![0u8; 42];
            b[..8].copy_from_slice(b"MPLS0300");
            b[8..12].copy_from_slice(&32u32.to_be_bytes());
            b[38..40].copy_from_slice(&count.to_be_bytes());
            for _ in 0..count {
                b.extend_from_slice(&20u16.to_be_bytes());
                let mut item = [0u8; 20];
                item[..5].copy_from_slice(b"00001");
                item[12..16].copy_from_slice(&63000u32.to_be_bytes());
                item[16..20].copy_from_slice(&333000u32.to_be_bytes());
                b.extend_from_slice(&item);
            }
            b
        };
        let mpls = temp.path().join("BDMV/PLAYLIST/00000.mpls");
        std::fs::write(&mpls, playlist(1)).unwrap();
        let stream = temp.path().join("BDMV/STREAM/00001.m2ts");
        std::fs::copy(fixture_dir().join("disc-clear.m2ts"), &stream).unwrap();
        let registry = mvc::Registry::new(&root.join("runtime/cine_mvc.dll"));
        let info = registry
            .prepare_disc(temp.path().to_str().unwrap(), 0, "cine-disc://test")
            .unwrap();
        assert!((info.duration - 6.0).abs() < 0.001);
        std::fs::write(&mpls, playlist(2)).unwrap();
        assert!(
            registry
                .prepare_disc(temp.path().to_str().unwrap(), 0, "cine-disc://bad")
                .is_err()
        );
        std::fs::write(&mpls, playlist(1)).unwrap();
        std::fs::write(&stream, vec![0xa5u8; 65536]).unwrap();
        assert!(
            registry
                .prepare_disc(temp.path().to_str().unwrap(), 0, "cine-disc://bad")
                .is_err()
        );
    }
    #[test]
    fn all_context_actions_are_reachable_from_the_four_toolbar_menus() {
        fn leaves(entries: &[popup::Entry], out: &mut std::collections::BTreeMap<u32, String>) {
            for e in entries {
                if !e.children.is_empty() {
                    leaves(&e.children, out);
                } else if e.enabled && e.id > 0 {
                    out.insert(e.id, e.label.clone());
                }
            }
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut s = State::new(root, true);
        player::load(
            &m,
            &mut s,
            fixture_dir().join("metadata-qa.mkv").to_str().unwrap(),
            None,
        )
        .unwrap();
        pump(&mut m, &mut s, |m, s| {
            !s.pending_load && m.get("track-list/count").as_deref() == Some("5")
        });
        s.titles.push(mvc::Title::default());
        let full = build_menu(&m, &s, false).unwrap().0;
        let remaining = build_menu(&m, &s, true).unwrap().0;
        let (audio, subs, _) = track_menus(&m).unwrap();
        let mut all = std::collections::BTreeMap::new();
        let mut toolbar = std::collections::BTreeMap::new();
        leaves(&full.entries().unwrap(), &mut all);
        for menu in [remaining, player::stereo_menu(&m, &s).unwrap(), audio, subs] {
            leaves(&menu.entries().unwrap(), &mut toolbar);
        }
        assert_eq!(
            all, toolbar,
            "An action was omitted or renamed in one access path"
        );
        let labels = full.labels();
        let hdr = labels.iter().position(|l| l == "Color / HDR").unwrap();
        assert_eq!(labels[hdr + 1], "Calidad / consumo");
        assert!(
            !player::stereo_menu(&m, &s)
                .unwrap()
                .labels()
                .iter()
                .any(|l| l == "Calidad / consumo")
        );
        assert!(
            all.values()
                .any(|l| l.starts_with("Español") && l.contains("AAC"))
        );
        assert!(all.values().any(|l| l.contains("SRT")));
    }
    #[test]
    fn low_consumption_limits_resources_and_restores_every_existing_profile() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("pause", "yes").unwrap();
        m.option("autofit", "640x360").unwrap();
        m.init().unwrap();
        let mut s = State::new(root, true);
        player::load(
            &m,
            &mut s,
            fixture_dir().join("ass3d-hsbs.mkv").to_str().unwrap(),
            None,
        )
        .unwrap();
        pump(&mut m, &mut s, |m, s| {
            !s.pending_load && m.get("video-params/w").is_some()
        });
        s.prefs.layout = Layout::HalfSbs;
        s.prefs.output = Output::Anaglyph;
        s.prefs.ass_mode = cineplayer_core::ass3d::Mode::Off;
        let keys = [
            "demuxer-max-bytes",
            "demuxer-max-back-bytes",
            "scale",
            "dscale",
            "hdr-compute-peak",
            "hwdec",
            "tone-mapping",
        ];
        for old in [
            cineplayer_core::stereo::Quality::Balanced,
            cineplayer_core::stereo::Quality::High,
            cineplayer_core::stereo::Quality::Compatibility,
        ] {
            s.prefs.quality = old;
            s.applied = None;
            apply(&m, &mut s).unwrap();
            settle(&mut m, &mut s);
            let original = keys.map(|k| m.get(k));
            let before = m.screenshot_pixels_kind("window").unwrap();
            player::action(&m, &mut s, 1053).unwrap();
            settle(&mut m, &mut s);
            assert_eq!(m.get("scale").as_deref(), Some("bilinear"));
            assert_eq!(m.get("hdr-compute-peak").as_deref(), Some("no"));
            assert!(
                m.get("demuxer-max-bytes").unwrap().parse::<u64>().unwrap() <= 16 * 1024 * 1024
            );
            assert_eq!(m.get("demuxer-max-back-bytes").as_deref(), Some("0"));
            assert_eq!(m.get("video-dec-params/w").as_deref(), Some("1920"));
            s.prefs.quality = old;
            s.applied = None;
            apply(&m, &mut s).unwrap();
            settle(&mut m, &mut s);
            assert_eq!(
                keys.map(|k| m.get(k)),
                original,
                "Low settings leaked into {old:?}"
            );
            let after = m.screenshot_pixels_kind("window").unwrap();
            assert_eq!((before.0, before.1), (after.0, after.1));
            let changed = before
                .2
                .as_chunks::<4>()
                .0
                .iter()
                .zip(after.2.as_chunks::<4>().0.iter())
                .filter(|(a, b)| a != b)
                .count();
            assert!(
                changed <= (before.0 * before.1 / 100).max(30),
                "Rendered output was not restored: {old:?}, changed {changed}"
            );
        }
    }
    #[test]
    fn toolbar_menu_keeps_all_other_context_options_including_disc_titles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut state = State::new(root, true);
        state.titles.push(mvc::Title::default());
        let all = build_menu(&m, &state, false).unwrap().0.labels();
        let remaining = build_menu(&m, &state, true).unwrap().0.labels();
        let expected: Vec<_> = all
            .into_iter()
            .filter(|s| {
                !s.starts_with("Vídeo 3D") && s != "Pista de audio" && s != "Pista de subtítulos"
            })
            .collect();
        assert_eq!(remaining, expected);
        assert!(remaining.iter().any(|s| s.starts_with("Títulos Blu-ray")));
        assert!(remaining.iter().any(|s| s == "Acerca de"));
    }
    #[test]
    fn native_fullscreen_covers_monitor_and_software_decode_is_available() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("hwdec", "no").unwrap(); // exercise the final fallback path from startup
        m.option("pause", "no").unwrap();
        m.init().unwrap();
        let file = fixture_dir().join("fullsbs.mkv");
        m.command(&["loadfile", file.to_str().unwrap()]).unwrap();
        fn wait_for(m: &mut Mpv, condition: impl Fn(&Mpv) -> bool) {
            let limit = Instant::now() + Duration::from_secs(10);
            while Instant::now() < limit {
                m.wait(0.05);
                if condition(m) {
                    return;
                }
            }
            panic!(
                "Fullscreen/software state not reached: {:?}; {:?}",
                m.get("time-pos"),
                windows::display_geometry(owner(m))
            );
        }
        wait_for(&mut m, |m| {
            m.get("time-pos")
                .and_then(|v| v.parse::<f64>().ok())
                .is_some_and(|v| v > 0.2)
        });
        assert_eq!(m.get("hwdec-current").as_deref(), Some("no"));
        println!(
            "Software verified; initial geometry: {:?}",
            windows::display_geometry(owner(&m))
        );
        for renderer in ["gpu-next", "gpu"] {
            m.set("vo", renderer).unwrap();
            for _ in 0..2 {
                m.set("fullscreen", "yes").unwrap();
                wait_for(&mut m, |m| {
                    windows::display_geometry(owner(m))
                        .is_some_and(|g| g["client"] == g["monitor"] && g["borderStyle"] == 0)
                });
                let geometry = windows::display_geometry(owner(&m)).unwrap();
                assert_eq!(geometry["perMonitorV2"], true, "{geometry}");
                println!("Fullscreen {renderer}: {geometry}");
                m.set("fullscreen", "no").unwrap();
                wait_for(&mut m, |m| {
                    windows::display_geometry(owner(m)).is_some_and(|g| g["borderStyle"] != 0)
                });
            }
        }
    }
    #[test]
    fn live_glasses_inversion_2d_and_full_sbs_change_rendered_pixels() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let samples = fixture_dir();
        for (sample, layout, has_ass) in [
            ("fullsbs.mkv", Layout::FullSbs, false),
            ("ass3d-hsbs.mkv", Layout::HalfSbs, true),
            ("ass3d-hou.mkv", Layout::HalfOu, true),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
            configure(&m, true, root).unwrap();
            m.init().unwrap();
            for (i, p) in ["sid", "sub-ass-extradata", "sub-text/ass-full"]
                .iter()
                .enumerate()
            {
                m.observe_string(p, 100 + i as u64).unwrap();
            }
            let mut s = State::new(temp.path(), true);
            s.prefs.layout = layout;
            s.prefs.output = Output::Anaglyph;
            player::load(&m, &mut s, samples.join(sample).to_str().unwrap(), None).unwrap();
            pump(&mut m, &mut s, |m, s| {
                !s.pending_load
                    && m.get("time-pos").is_some()
                    && (!has_ass
                        || s.subtitles
                            .active(s.prefs.ass_mode, s.effective(m))
                            .is_some())
            });
            let mut previous = None;
            for id in 1200..=1204 {
                player::action(&m, &mut s, id).unwrap();
                settle(&mut m, &mut s);
                let shot = m.screenshot_pixels().unwrap();
                if let Some(old) = previous {
                    assert!(
                        changed_pixels(&old, &shot) > 30,
                        "Gafas {id}: no cambian los píxeles de {sample}"
                    );
                }
                previous = Some(shot);
            }
            player::action(&m, &mut s, 1200).unwrap();
            settle(&mut m, &mut s);
            let before = m.screenshot_pixels().unwrap();
            player::action(&m, &mut s, 1024).unwrap();
            settle(&mut m, &mut s);
            let swapped = m.screenshot_pixels().unwrap();
            assert!(
                changed_pixels(&before, &swapped) > 30,
                "Invertir no cambia {sample}"
            );
            player::action(&m, &mut s, 1210).unwrap();
            settle(&mut m, &mut s);
            assert_eq!(s.prefs.output, Output::Left);
            assert!(!s.prefs.swap);
            let left = m.screenshot_pixels().unwrap();
            assert!(
                changed_pixels(&swapped, &left) > 30,
                "2D no cambia {sample}"
            );
            for (id, w, h) in [(1221, 16, 9), (1222, 1920, 2205)] {
                player::action(&m, &mut s, id).unwrap();
                settle(&mut m, &mut s);
                let shot = m.screenshot_pixels().unwrap();
                // Legacy gpu screenshots use the source display size (video.c:
                // gl_video_screenshot), independently of MAIN texture dimensions.
                // Verify the declared target texture, executed pass and final aspect.
                if id == 1222 {
                    let source = std::fs::read_to_string(m.get("glsl-shaders").unwrap()).unwrap();
                    assert!(source.contains(&format!("//!WIDTH {w}\n//!HEIGHT {h}")));
                    assert!(
                        m.get("vo-passes")
                            .unwrap()
                            .contains("Enigmax Frame Packing")
                    );
                } else {
                    assert!(!m.get("glsl-shaders").unwrap_or_default().contains("3840"));
                }
                assert!(
                    (shot.0 as f64 / shot.1 as f64 - w as f64 / h as f64).abs() < 0.01,
                    "Aspecto Full SBS {sample}"
                );
                if id == 1222 {
                    let gap = ((shot.1 as f64 * 0.5) as usize).min(shot.1 - 1);
                    assert!(
                        shot.2[gap * shot.0 * 4..(gap + 1) * shot.0 * 4]
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .all(|p| p[..3].iter().all(|v| *v < 5)),
                        "Separación FramePack no negra"
                    );
                    if !has_ass {
                        let pixel = |s: &(usize, usize, Vec<u8>), y: f64| {
                            let i =
                                ((s.1 as f64 * y) as usize * s.0 + (s.0 as f64 * 0.2) as usize) * 4;
                            s.2[i..i + 3].to_vec()
                        };
                        let upper = pixel(&shot, 0.25);
                        let lower = pixel(&shot, 0.75);
                        assert!(upper.iter().zip(&lower).any(|(a, b)| a.abs_diff(*b) > 80));
                        player::action(&m, &mut s, 1024).unwrap();
                        settle(&mut m, &mut s);
                        let swapped = m.screenshot_pixels().unwrap();
                        assert!(
                            upper
                                .iter()
                                .zip(pixel(&swapped, 0.75))
                                .all(|(a, b)| a.abs_diff(b) < 5)
                        );
                        assert!(
                            lower
                                .iter()
                                .zip(pixel(&swapped, 0.25))
                                .all(|(a, b)| a.abs_diff(b) < 5)
                        );
                        player::action(&m, &mut s, 1024).unwrap();
                        settle(&mut m, &mut s);
                    }
                }
            }
            player::action(&m, &mut s, 1020).unwrap();
            settle(&mut m, &mut s);
            assert_eq!(s.prefs.delivery, cineplayer_core::stereo::Delivery::Native);
            assert!(
                m.get("glsl-shaders").unwrap_or_default().is_empty(),
                "Original debe evitar la textura de resolución fija"
            );
            println!(
                "{sample}: cinco gafas, inversión, 2D y ambos tamaños Full SBS verificados por píxeles"
            );
        }
    }
    #[test]
    fn each_user_open_resets_view_but_keeps_glasses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut s = State::new(root, false);
        s.prefs.layout = Layout::FullSbs;
        s.prefs.output = Output::Anaglyph;
        s.prefs.glasses = Glasses::GreenMagenta;
        s.prefs.swap = true;
        s.view_choice_made = true;
        s.new_open();
        assert!(!s.view_choice_made);
        assert_eq!(s.prefs.layout, Layout::Auto);
        assert_eq!(s.prefs.output, Output::Left);
        assert_eq!(s.prefs.glasses, Glasses::GreenMagenta);
        assert!(!s.prefs.swap);
        s.view_choice_made = true;
        s.new_open();
        assert!(!s.view_choice_made);
        assert!(APP_NAME_VERSION.ends_with(VERSION));
    }
    fn pump(m: &mut Mpv, s: &mut State, condition: impl Fn(&Mpv, &State) -> bool) {
        let limit = Instant::now() + Duration::from_secs(10);
        while Instant::now() < limit {
            let event = m.wait(0.05);
            let (id, cookie) = (event.id, event.userdata);
            if id == 5 && cookie & (1_u64 << 63) != 0 {
                m.seek_reply(cookie).unwrap();
            }
            if id == 8 {
                s.pending_load = false;
            }
            let changed = if id == 22 || id == 8 {
                player::subtitle_event(m, s).unwrap()
            } else {
                false
            };
            if id == 8 || id == 17 || changed {
                apply(m, s).unwrap();
            }
            s.restore_tracks(m).unwrap();
            s.capture_tracks(m);
            s.restore_position(m).unwrap();
            s.remember_ready(m).unwrap();
            if condition(m, s) {
                return;
            }
        }
        panic!(
            "No se alcanzó el estado esperado: {} · VO {:?} · posición {:?} · pausa {:?} · archivo {:?} · geometría {:?}",
            s.subtitles.status(s.prefs.ass_mode, s.effective(m)),
            m.get("current-vo"),
            m.get("time-pos"),
            m.get("pause"),
            m.get("path"),
            windows::display_geometry(owner(m)),
        );
    }
    #[test]
    fn asynchronous_seek_tracks_stop_reload_and_quit() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut s = State::new(root, true);
        m.seek(10.0, false).unwrap(); // no file: no command and no error dialog
        assert!(m.seek(f64::NAN, true).is_err());
        let file = fixture_dir().join("pistas-qa.mkv");
        for round in 0..3 {
            player::load(&m, &mut s, file.to_str().unwrap(), None).unwrap();
            pump(&mut m, &mut s, |m, s| {
                !s.pending_load && m.get("track-list/count").as_deref() == Some("5")
            });
            let (_, _, tracks) = track_menus(&m).unwrap();
            assert_eq!(
                tracks
                    .iter()
                    .filter(|(_, a)| matches!(a, Action::Track(..)))
                    .count(),
                5
            ); // two audio, two subtitles, subtitles off
            action(&m, &mut s, Action::Track("aid".into(), "2".into())).unwrap();
            action(&m, &mut s, Action::Track("sid".into(), "2".into())).unwrap();
            pump(&mut m, &mut s, |m, _| {
                m.get("aid").as_deref() == Some("2") && m.get("sid").as_deref() == Some("2")
            });
            let start = Instant::now();
            for i in 0..100 {
                m.seek((i % 20) as f64, true).unwrap();
            }
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "Las búsquedas no deben esperar a la decodificación"
            );
            pump(&mut m, &mut s, |m, _| {
                m.get("time-pos")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| (v - 19.0).abs() < 0.15)
            });
            m.seek(1e8, true).unwrap();
            pump(&mut m, &mut s, |m, _| {
                m.get("time-pos")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| (29.8..=30.1).contains(&v))
            });
            action(&m, &mut s, Action::Track("sid".into(), "no".into())).unwrap();
            pump(&mut m, &mut s, |m, _| m.get("sid").as_deref() == Some("no"));
            m.seek(1.0, true).unwrap();
            m.command_async(&["stop"], 0).unwrap();
            m.reset_seeks();
            let until = Instant::now() + Duration::from_secs(5);
            while m.get("idle-active").as_deref() != Some("yes") && Instant::now() < until {
                m.wait(0.05);
            }
            assert_eq!(
                m.get("idle-active").as_deref(),
                Some("yes"),
                "Cierre de archivo en ciclo {round}"
            );
            m.seek(10.0, false).unwrap();
        }
        action(&m, &mut s, Action::Id(55)).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let event = m.wait(0.05);
            if event.id == 1 {
                break;
            }
            assert!(
                Instant::now() < until,
                "El cierre debe completar sin diálogos"
            );
        }
    }
    #[test]
    #[ignore = "Requires ENIGMAX_TEST_MVC_ISO pointing to an explicitly authorized user ISO"]
    fn mvc_seeks_and_quit_during_decode() {
        let path = std::env::var("ENIGMAX_TEST_MVC_ISO").expect("Provide authorized ISO path");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        m.register_mvc(Box::new(mvc::Registry::new(
            &root.join("runtime/cine_mvc.dll"),
        )))
        .unwrap();
        let mut s = State::new(root, true);
        s.prefs.layout = Layout::Mvc;
        s.prefs.output = Output::Anaglyph;
        player::load(&m, &mut s, &path, None).unwrap();
        pump(&mut m, &mut s, |m, s| {
            !s.pending_load && s.raw.is_some() && m.get("time-pos").is_some()
        });
        assert!(
            !chapter_entries(&m).is_empty(),
            "La ISO MVC pierde los capítulos del título"
        );
        println!("Capítulos MVC importados: {}", chapter_entries(&m).len());
        let start = Instant::now();
        for i in 0..100 {
            m.seek((i % 60 + 1) as f64, true).unwrap();
        }
        println!("MVC: 100 seek requests submitted in {:?}", start.elapsed());
        pump(&mut m, &mut s, |m, _| {
            m.get("time-pos")
                .and_then(|v| v.parse::<f64>().ok())
                .is_some_and(|v| (v - 40.0).abs() < 0.15)
        });
        m.seek(120.0, true).unwrap();
        let shutdown = Instant::now();
        action(&m, &mut s, Action::Id(55)).unwrap();
        loop {
            if m.wait(0.05).id == 1 {
                break;
            }
            assert!(
                shutdown.elapsed() < Duration::from_secs(10),
                "MVC no terminó durante el cierre"
            );
        }
        drop(m);
        assert!(shutdown.elapsed() < Duration::from_secs(10));
        println!(
            "MVC quit including decoder destruction: {:?}",
            shutdown.elapsed()
        );
    }
    #[test]
    fn ass_stereo_track_switch_seek_and_external_subtitles() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        for (i, key) in ["sid", "sub-ass-extradata", "sub-text/ass-full"]
            .iter()
            .enumerate()
        {
            m.observe_string(key, i as u64).unwrap();
        }
        let mut s = State::new(root, true);
        s.prefs.layout = Layout::HalfSbs;
        s.prefs.output = Output::Anaglyph;
        player::load(
            &m,
            &mut s,
            fixture_dir().join("ass3d-hsbs.mkv").to_str().unwrap(),
            None,
        )
        .unwrap();
        let active = |m: &Mpv, s: &State| {
            s.subtitles
                .active(s.prefs.ass_mode, s.effective(m))
                .is_some()
                && m.get("current-vo").as_deref() == Some("gpu")
        };
        pump(&mut m, &mut s, active);
        assert_eq!(m.get("blend-subtitles").as_deref(), Some("video"));
        let generation = s.generation;
        player::action(&m, &mut s, 1201).unwrap();
        assert_eq!(s.prefs.output, Output::Anaglyph);
        assert_eq!(s.prefs.glasses, Glasses::GreenMagenta);
        assert_eq!(
            s.generation, generation,
            "La elección de gafas no debe recargar el archivo SBS"
        );
        player::action(&m, &mut s, 1210).unwrap();
        assert_eq!(s.prefs.output, Output::Left);
        assert_eq!(m.get("sid").as_deref(), Some("1"));
        player::action(&m, &mut s, 1200).unwrap();
        assert_eq!(s.prefs.output, Output::Anaglyph);
        assert_eq!(s.prefs.glasses, Glasses::RedCyan);
        assert_eq!(
            m.get("sub-ass-video-aspect-override").as_deref(),
            Some("0.500000")
        );
        m.set("sid", "2").unwrap();
        pump(&mut m, &mut s, |m, s| {
            s.subtitles.header.is_none() && m.get("current-vo").as_deref() == Some("gpu")
        });
        assert_eq!(m.get("blend-subtitles").as_deref(), Some("video"));
        assert_eq!(m.get("cine-sub-packed").as_deref(), Some("no"));
        assert_eq!(m.get("cine-sub-layout").as_deref(), Some("1"));
        m.set("sid", "1").unwrap();
        pump(&mut m, &mut s, active);
        m.command(&["seek", "1", "absolute+exact"]).unwrap();
        pump(&mut m, &mut s, |m, s| {
            active(m, s)
                && m.get("time-pos")
                    .is_some_and(|v| v.parse::<f64>().unwrap_or(0.0) >= 0.9)
        });
        m.set("sid", "no").unwrap();
        pump(&mut m, &mut s, |m, s| {
            s.subtitles.header.is_none() && m.get("current-vo").as_deref() == Some("gpu-next")
        });
        player::load(
            &m,
            &mut s,
            fixture_dir().join("ass3d-black.mkv").to_str().unwrap(),
            None,
        )
        .unwrap();
        pump(&mut m, &mut s, |m, s| {
            !s.pending_load && m.get("track-list/count").as_deref() == Some("1")
        });
        m.command(&[
            "sub-add",
            fixture_dir().join("ass3d-sbs.ass").to_str().unwrap(),
            "select",
        ])
        .unwrap();
        pump(&mut m, &mut s, active);
        player::action(&m, &mut s, 1022).unwrap();
        assert_eq!(m.get("track-list/count").as_deref(), Some("2"));
        assert_eq!(m.get("sid").as_deref(), Some("1"));
        assert_eq!(m.get("video-crop").as_deref(), Some(""));
        assert!(m.get("glsl-shaders").is_some_and(|v| !v.is_empty()));
        for (sample, layout, expected_dw, expected_dh) in [
            ("ass3d-tagged-fullsbs.mkv", Layout::FullSbs, "3840", "1080"),
            ("ass3d-tagged-fullou.mkv", Layout::FullOu, "1920", "2160"),
            ("ass3d-hsbs.mkv", Layout::HalfSbs, "1920", "1080"),
            ("ass3d-hou.mkv", Layout::HalfOu, "1920", "1080"),
        ] {
            s.prefs.layout = layout;
            s.prefs.output = Output::Original;
            player::load(
                &m,
                &mut s,
                fixture_dir().join(sample).to_str().unwrap(),
                None,
            )
            .unwrap();
            pump(&mut m, &mut s, |m, s| {
                !s.pending_load
                    && m.get("video-out-params/dw").as_deref() == Some(expected_dw)
                    && m.get("video-out-params/dh").as_deref() == Some(expected_dh)
                    && m.get("video-out-params/stereo-in").as_deref() == Some("mono")
            });
            assert_eq!(m.get("video-out-params/stereo-in").as_deref(), Some("mono"));
        }
    }
}
