#![windows_subsystem = "windows"]
mod controls;
mod mpv;
mod mvc;
mod player;
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
        ("vo", "gpu-next"),
        ("gpu-api", "d3d11"),
        ("gpu-context", "d3d11"),
        ("hwdec", "auto"),
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
    let script = dir.join("assets/enigmax_controls.lua");
    if !script.is_file() {
        return Err("Falta el módulo de controles inferiores".into());
    }
    m.option("scripts", script.to_str().ok_or("Ruta Unicode inválida")?)?;
    m.option("script-opts", &format!("enigmax-client={}", m.name()))?;
    if probe {
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
    ] {
        m.command(&[
            "keybind",
            key,
            &format!("script-message-to {name} {action}"),
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
fn help(m: &Mpv) -> Result<(), String> {
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
    subs.item(
        300,
        "Desactivar subtítulos",
        m.get("sid").as_deref() == Some("no"),
    )?;
    actions.push((300, Action::Track("sid".into(), "no".into())));
    let count = m
        .get("track-list/count")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(0)
        .min(256);
    for i in 0..count {
        let prefix = format!("track-list/{i}");
        let kind = m.get(&format!("{prefix}/type")).unwrap_or_default();
        if kind != "audio" && kind != "sub" {
            continue;
        }
        let Some(track) = m.get(&format!("{prefix}/id")) else {
            continue;
        };
        let title = m.get(&format!("{prefix}/title")).unwrap_or_default();
        let lang = m.get(&format!("{prefix}/lang")).unwrap_or_default();
        let codec = m.get(&format!("{prefix}/codec")).unwrap_or_default();
        let id = 400 + i as u32;
        let label = format!("{track}: {lang} {title} [{codec}]");
        let selected = m.get(&format!("{prefix}/selected")).as_deref() == Some("yes");
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
fn menu(m: &Mpv, s: &State) -> Result<Option<Action>, String> {
    let root = windows::Menu::new()?;
    root.item(1, "Abrir vídeo…\tCtrl+O", false)?;
    root.item(2, "Añadir subtítulos…\tCtrl+S", false)?;
    root.separator();
    player::menu(&root, m, s)?;
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
    let audio = windows::Menu::new()?;
    for (id, label, value) in [
        (40, "TV / altavoces: decodificar a PCM", 0),
        (41, "Receptor: AC3 / DTS", 1),
        (42, "Receptor HDMI: AC3 / EAC3 / DTS-HD / TrueHD", 2),
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
    let (tracks, subs, track_actions) = track_menus(m)?;
    actions.extend(track_actions);
    root.submenu("Pista de audio", tracks)?;
    root.submenu("Pista de subtítulos", subs)?;
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
            s.new_open();
            player::load(m, s, value, None)?;
        }
    }
    Ok(())
}
fn original(m: &Mpv) -> Result<(), String> {
    m.set("fullscreen", "no")?;
    m.set("window-scale", "1")
}
fn info(m: &Mpv, s: &State) {
    let mut report = format!(
        "{APP_NAME_VERSION}\nPresentación: {} · {}\n{}\n\n",
        s.prefs.layout.label(),
        s.prefs.output.label(),
        s.pipeline
    );
    for p in [
        "mpv-version",
        "ffmpeg-version",
        "video-format",
        "video-params/w",
        "video-params/h",
        "video-params/primaries",
        "video-params/gamma",
        "video-out-params/w",
        "video-out-params/h",
        "hwdec-current",
        "display-colorspace",
        "display-hidpi-scale",
        "audio-codec-name",
        "audio-params/format",
        "audio-out-params/format",
        "current-ao",
        "frame-drop-count",
        "decoder-frame-drop-count",
        "demuxer-cache-state/cache-bytes",
    ] {
        report.push_str(&format!(
            "{p}: {}\n",
            m.get(p).unwrap_or_else(|| "No disponible".into())
        ));
    }
    report.push_str("\nHDR10+ / Dolby Vision: no se garantiza envío de metadatos propietarios.\nEl modo HDR automático requiere la configuración HDR adecuada en Windows.");
    windows::message(&report, false);
}
fn action(m: &Mpv, s: &mut State, a: Action) -> Result<(), String> {
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
                windows::message(
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
            55 => {
                m.reset_seeks();
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
    for (i, p) in ["sid", "sub-ass-extradata", "sub-text/ass-full"]
        .iter()
        .enumerate()
    {
        m.observe_string(p, 100 + i as u64)?;
    }
    m.register_mvc(Box::new(mvc::Registry::new(
        &dir.join("runtime/cine_mvc.dll"),
    )))?;
    bind(&m)?;
    let mut state = State::new(dir, probe);
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
        } else if parts[0] == "original" || parts.contains(&"both") {
            Output::Original
        } else {
            Output::Left
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
            state.prefs.convergence = 0.01;
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
        if parts.contains(&"cpu") {
            state.prefs.quality = cineplayer_core::stereo::Quality::Compatibility;
        }
        state.hdr_sdr = parts.contains(&"sdr");
        output = Some(PathBuf::from(&args[3]));
        Some(&args[1])
    } else {
        if args.len() > 1 {
            return Err("Abre un único vídeo cada vez".into());
        }
        args.first()
    };
    if let Some(file) = file {
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
    } else {
        help(&m)?;
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
        // No polling in normal playback: Rust sleeps until libmpv delivers an event.
        let e = m.wait(if probe { 0.2 } else { -1.0 });
        let id = e.id;
        let reply = (e.userdata, e.error);
        // Do not issue more commands after quit. END_FILE/QUIT can precede
        // SHUTDOWN while older control messages are still queued.
        if id == 1
            || (id == 7 && !e.data.is_null() && unsafe { (*e.data.cast::<EndFile>()).reason } == 3)
        {
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
        if let Some(err) = end_error
            && !redirecting
        {
            let detail = m.mvc()?.error();
            let msg = format!("No se pudo reproducir el archivo (libmpv {err}). {detail}");
            if probe {
                return Err(msg);
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
            if let Some(pos) = state.resume.take().or_else(|| {
                if probe {
                    std::env::var("ENIGMAX_PROBE_START")
                        .ok()
                        .filter(|v| !v.is_empty())
                } else {
                    None
                }
            }) && let Ok(pos) = pos.parse::<f64>()
            {
                m.seek(pos, true)?;
            }
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
                                controls::Input::Audio => choose_track(&m, &mut state, false),
                                controls::Input::Subtitles => choose_track(&m, &mut state, true),
                                controls::Input::Menu => player::stereo_menu(&m, &state)
                                    .and_then(|menu| menu.show(&m))
                                    .and_then(|id| {
                                        if id == 0 {
                                            Ok(())
                                        } else {
                                            action(&m, &mut state, Action::Id(id))
                                        }
                                    }),
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
                "menu" => menu(&m, &state).and_then(|a| {
                    if let Some(a) = a {
                        action(&m, &mut state, a)
                    } else {
                        Ok(())
                    }
                }),
                "open" => open(&m, &mut state, false),
                "sub" => open(&m, &mut state, true),
                "original" => original(&m),
                "help" => help(&m),
                _ => Ok(()),
            };
            if let Err(e) = result {
                if probe {
                    return Err(e);
                }
                runtime_error(&m, dir, "Acción del usuario", &e);
            }
        }
        if id == 8 || id == 17 || subs_changed {
            player::ask_view(&m, &mut state)?;
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
                    if ui { "window" } else { "subtitles" },
                ])?;
                let report = [
                    "title",
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
                    "video-params/primaries",
                    "video-params/gamma",
                    "target-trc",
                    "target-prim",
                    "sid",
                    "sub-text",
                    "audio-codec-name",
                    "track-list",
                ]
                .iter()
                .map(|p| format!("{p}: {}", m.get(p).unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("\n");
                std::fs::write(
                    path.with_extension("txt"),
                    format!(
                        "{}\nMPLS: {}\nMVC: {:?}\nPipeline: {}\nError MVC: {}\nASS 3D: {}",
                        report,
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
            if condition(m, s) {
                return;
            }
        }
        panic!(
            "No se alcanzó el estado esperado: {} · VO {:?}",
            s.subtitles.status(s.prefs.ass_mode, s.effective(m)),
            m.get("current-vo")
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
        let file = root.join("verificacion/muestras/pistas-qa.mkv");
        for round in 0..3 {
            player::load(&m, &mut s, file.to_str().unwrap(), None).unwrap();
            pump(&mut m, &mut s, |m, s| {
                !s.pending_load && m.get("track-list/count").as_deref() == Some("5")
            });
            let (_, _, tracks) = track_menus(&m).unwrap();
            assert_eq!(tracks.len(), 5); // two audio, two subtitles, subtitles off
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
            root.join("verificacion/muestras/ass3d-hsbs.mkv")
                .to_str()
                .unwrap(),
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
            s.subtitles.header.is_none() && m.get("current-vo").as_deref() == Some("gpu-next")
        });
        assert_eq!(m.get("blend-subtitles").as_deref(), Some("no"));
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
            root.join("verificacion/muestras/ass3d-black.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        pump(&mut m, &mut s, |m, _| {
            m.get("track-list/count").as_deref() == Some("1")
        });
        m.command(&[
            "sub-add",
            root.join("verificacion/muestras/ass3d-sbs.ass")
                .to_str()
                .unwrap(),
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
                root.join("verificacion/muestras")
                    .join(sample)
                    .to_str()
                    .unwrap(),
                None,
            )
            .unwrap();
            pump(&mut m, &mut s, |m, s| {
                !s.pending_load
                    && m.get("video-out-params/dw").as_deref() == Some(expected_dw)
                    && m.get("video-out-params/dh").as_deref() == Some(expected_dh)
            });
            assert_eq!(m.get("video-out-params/stereo-in").as_deref(), Some("mono"));
        }
    }
}
