//! Pixel-level rendering regressions, separate from physical display calibration.
use crate::{
    configure,
    mpv::Mpv,
    player::{self, State},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn pump(m: &mut Mpv, time: Duration) {
    let until = Instant::now() + time;
    while Instant::now() < until {
        m.wait(0.02);
    }
}
#[test]
fn srt_styles_change_pixels_while_ass_and_pgs_authoring_is_preserved() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    m.option("autofit", "640x360").unwrap();
    m.option("hwdec", "no").unwrap();
    m.init().unwrap();
    let mut s = State::new(temp.path(), true);
    player::load(
        &m,
        &mut s,
        root.join("verificacion/muestras/ass3d-black.mkv")
            .to_str()
            .unwrap(),
        None,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if m.wait(0.02).id == 8 {
            break;
        }
        assert!(Instant::now() < until);
    }
    s.pending_load = false;
    pump(&mut m, Duration::from_millis(200));
    let source = m.get("video-params");
    for ext in ["srt", "ass"] {
        m.command(&[
            "sub-add",
            root.join(format!("verificacion/muestras/depth-qa.{ext}"))
                .to_str()
                .unwrap(),
            "select",
        ])
        .unwrap();
        s.features.invalidate();
        s.features.prefs.sub_color = "#FFFF00".into();
        let mut counts = vec![];
        let mut images = vec![];
        for scale in [1.0, 1.5] {
            s.features.prefs.sub_scale = scale;
            s.features.apply(&m, 0).unwrap();
            pump(&mut m, Duration::from_millis(200));
            let (_, _, pixels) = m.screenshot_pixels().unwrap();
            let yellow = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] > 180 && p[1] > 180 && p[2] < 70)
                .count();
            if ext == "srt" {
                assert!(yellow > 50, "SRT color was not applied: {yellow}");
            }
            images.push(pixels);
            counts.push(yellow);
            assert_eq!(m.get("video-params"), source);
        }
        if ext == "srt" {
            assert!(
                counts[1] > counts[0] * 3 / 2,
                "SRT did not grow: {counts:?}"
            );
        } else {
            assert_eq!(
                images[0], images[1],
                "ASS positions, size or authored color changed"
            );
            assert_eq!(m.get("sub-ass-override").as_deref(), Some("no"));
        }
    }
    player::load(
        &m,
        &mut s,
        root.join("verificacion/muestras/depth-qa-pgs.mkv")
            .to_str()
            .unwrap(),
        None,
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if m.wait(0.02).id == 8 {
            break;
        }
        assert!(Instant::now() < until);
    }
    m.set("sid", "1").unwrap();
    let mut counts = vec![];
    for scale in [1.0, 1.5] {
        s.features.prefs.sub_scale = scale;
        s.features.invalidate();
        s.features.apply(&m, 0).unwrap();
        pump(&mut m, Duration::from_millis(250));
        assert_eq!(
            m.get("sub-ass-override").as_deref(),
            Some("yes"),
            "PGS codec: {:?}",
            m.get("current-tracks/sub/codec")
        );
        let (_, _, pixels) = m.screenshot_pixels().unwrap();
        counts.push(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] > 180 && p[1] > 180 && p[2] > 180)
                .count(),
        );
    }
    assert!(
        counts[0] > 50 && counts[1] == counts[0],
        "PGS authored bitmap changed: {counts:?}"
    );
}
#[test]
fn gpu_sdr_ranges_and_hdr_to_sdr_gradients_are_neutral_and_monotonic() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    m.option("keepaspect", "no").unwrap();
    m.option("autofit", "512x128").unwrap();
    m.init().unwrap();
    let mut s = State::new(temp.path(), true);
    s.hdr_sdr = true;
    let mut rows = Vec::new();
    for name in ["limited", "full", "hdr"] {
        player::load(
            &m,
            &mut s,
            root.join(format!("verificacion/muestras/color-{name}.mkv"))
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if m.wait(0.02).id == 8 {
                break;
            }
            assert!(Instant::now() < until);
        }
        s.pending_load = false;
        pump(&mut m, Duration::from_millis(150));
        println!(
            "{name}: source before apply dec={:?}, normalized={:?}",
            m.get("video-dec-params"),
            m.get("video-params")
        );
        player::apply(&m, &mut s).unwrap();
        pump(&mut m, Duration::from_millis(400));
        assert_eq!(m.get("current-vo").as_deref(), Some("gpu-next"));
        assert_eq!(
            m.get("tone-mapping").as_deref(),
            Some(if name == "hdr" { "st2094-40" } else { "auto" })
        );
        assert_eq!(
            m.get("video-params/primaries").as_deref(),
            Some(if name == "hdr" { "bt.2020" } else { "bt.709" })
        );
        let (w, h, p) = m.screenshot_pixels_kind("window").unwrap();
        let row = (0..w)
            .map(|x| {
                let i = ((h / 2) * w + x) * 4;
                assert!(
                    p[i].abs_diff(p[i + 1]) <= 3 && p[i].abs_diff(p[i + 2]) <= 3,
                    "Non-neutral {name}/{x}"
                );
                p[i]
            })
            .collect::<Vec<_>>();
        assert!(row[0] < 8, "Raised blacks {name}: {}", row[0]);
        assert!(row[w - 1] > 220, "Low whites {name}: {}", row[w - 1]);
        assert!(
            row.windows(2).all(|v| v[1] as i16 - v[0] as i16 >= -3),
            "Non-monotonic ramp {name}"
        );
        let mut levels = row.clone();
        levels.sort_unstable();
        levels.dedup();
        assert!(
            levels.len() > 90,
            "Gradient collapsed {name}: {}",
            levels.len()
        );
        println!(
            "{name}: black={}, white={}, distinct output levels={}",
            row[0],
            row[w - 1],
            levels.len()
        );
        println!(
            "{name}: quarter values {:?}",
            [row[0], row[w / 4], row[w / 2], row[w * 3 / 4], row[w - 1]]
        );
        rows.push(row);
    }
    assert_eq!(rows[0].len(), rows[1].len());
    let mean = rows[0]
        .iter()
        .zip(&rows[1])
        .map(|(a, b)| a.abs_diff(*b) as f64)
        .sum::<f64>()
        / rows[0].len() as f64;
    assert!(
        mean < 1.5,
        "Incorrect full/limited range conversion: {mean}"
    );
    println!("SDR limited/full average difference: {mean}");
}
#[test]
fn preprojected_hud_is_not_duplicated_again_by_stereo_metadata() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for vo in ["gpu-next", "gpu"] {
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("vo", vo).unwrap();
        m.option("autofit", "640x360").unwrap();
        m.init().unwrap();
        m.command(&[
            "loadfile",
            root.join("verificacion/muestras/ass3d-black.mkv")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            if m.wait(0.02).id == 8 {
                break;
            }
            assert!(Instant::now() < until);
        }
        m.set("sid", "no").unwrap();
        m.command(&["vf", "set", "format=stereo-in=sbs2l"]).unwrap();
        pump(&mut m, Duration::from_millis(400));
        let w = m.get("osd-width").unwrap().parse::<usize>().unwrap();
        let pixels = [0u8, 0, 255, 255].repeat(40 * 12);
        for (id, x) in [(61, 12), (62, w / 2 + 18)] {
            m.command_named(
                "overlay-add",
                &[
                    ("id", id.to_string()),
                    ("x", x.to_string()),
                    ("y", "24".into()),
                    ("file", format!("&{}", pixels.as_ptr() as usize)),
                    ("offset", "0".into()),
                    ("fmt", "bgra".into()),
                    ("w", "40".into()),
                    ("h", "12".into()),
                    ("stride", "160".into()),
                ],
            )
            .unwrap();
        }
        pump(&mut m, Duration::from_millis(250));
        let (width, _, image) = m.screenshot_pixels_kind("window").unwrap();
        let count = (0..width)
            .filter(|x| {
                let i = (28 * width + x) * 4;
                image[i] > 220 && image[i + 1] < 40 && image[i + 2] < 40
            })
            .count();
        assert!(
            (78..=82).contains(&count),
            "{vo}: expected 80 red HUD pixels across both eyes, got {count}"
        );
        println!("{vo}: exactly two intended eye planes, red width {count}");
    }
}

#[test]
fn subtitle_renderer_switches_preserve_window_and_stereo_cursor_covers_hud() {
    use cineplayer_core::{
        ass3d::Mode,
        stereo::{Layout, Output},
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    m.option("autofit", "640x360").unwrap();
    m.init().unwrap();
    let mut s = State::new(temp.path(), true);
    player::load(
        &m,
        &mut s,
        root.join("verificacion/muestras/ass3d-black.mkv")
            .to_str()
            .unwrap(),
        None,
    )
    .unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if m.wait(0.02).id == 8 {
            break;
        }
        assert!(Instant::now() < end);
    }
    s.pending_load = false;
    pump(&mut m, Duration::from_millis(200));
    s.prefs.layout = Layout::FullSbs;
    s.prefs.output = Output::Original;
    s.prefs.ass_mode = Mode::Sbs;
    let before = crate::windows::display_geometry(crate::owner(&m)).unwrap()["client"].clone();
    for ext in ["srt", "ass", "srt", "ass"] {
        m.command(&[
            "sub-add",
            root.join(format!("verificacion/muestras/depth-qa.{ext}"))
                .to_str()
                .unwrap(),
            "select",
        ])
        .unwrap();
        pump(&mut m, Duration::from_millis(100));
        crate::subtitles::refresh(&m, &mut s);
        s.applied = None;
        player::apply(&m, &mut s).unwrap();
        let end = Instant::now() + Duration::from_millis(700);
        while Instant::now() < end {
            if m.wait(0.02).id == 17 {
                s.restore_window(&m);
            }
        }
        s.restore_window(&m);
        let after = crate::windows::display_geometry(crate::owner(&m)).unwrap()["client"].clone();
        assert_eq!(before, after, "Window geometry changed selecting {ext}");
        assert_eq!(
            m.get("current-vo").as_deref(),
            Some(if ext == "ass" { "gpu" } else { "gpu-next" })
        );
    }
    // Opaque HUD bitmap, then cursor. A later HUD repaint must stay BELOW it.
    let red = [0u8, 0, 255, 255].repeat(100 * 40);
    for (id, x, y) in [(61, 20, 40), (62, 180, 40)] {
        m.command_named(
            "overlay-add",
            &[
                ("id", id.to_string()),
                ("x", x.to_string()),
                ("y", y.to_string()),
                ("file", format!("&{}", red.as_ptr() as usize)),
                ("offset", "0".into()),
                ("fmt", "bgra".into()),
                ("w", "100".into()),
                ("h", "40".into()),
                ("stride", "400".into()),
            ],
        )
        .unwrap();
    }
    crate::controls::Controls::cursor(&m, "30|45|24|32|190|45|24|32").unwrap();
    pump(&mut m, Duration::from_millis(80));
    let (w, _, pixels) = m.screenshot_pixels_kind("window").unwrap();
    for x in [32, 192] {
        let i = (53 * w + x) * 4;
        assert!(
            pixels[i..i + 3].iter().all(|v| *v > 220),
            "Cursor hidden by HUD at {x}: {:?}",
            &pixels[i..i + 3]
        );
    }
    crate::controls::Controls::cursor(&m, "").unwrap();
}

#[test]
fn seeded_mode_track_seek_walk_preserves_window_pause_and_source_color() {
    use cineplayer_core::stereo::{Delivery, Layout, Output};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    m.option("autofit", "640x360").unwrap();
    m.init().unwrap();
    let mut s = State::new(temp.path(), true);
    let mut rng = 0x12345678u32;
    let mut steps = 0;
    for (file, layout) in [
        ("hsbs", Layout::HalfSbs),
        ("fullsbs", Layout::FullSbs),
        ("hou", Layout::HalfOu),
        ("fullou", Layout::FullOu),
    ] {
        player::load(
            &m,
            &mut s,
            root.join(format!("verificacion/muestras/ass3d-{file}.mkv"))
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if m.wait(0.02).id == 8 {
                break;
            }
            assert!(Instant::now() < end);
        }
        s.pending_load = false;
        pump(&mut m, Duration::from_millis(100));
        s.prefs.layout = layout;
        let geometry =
            crate::windows::display_geometry(crate::owner(&m)).unwrap()["client"].clone();
        let mut orders = vec![0, 1, 2, 3, 4, 0, 1, 2, 3, 4];
        for i in (1..orders.len()).rev() {
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            orders.swap(i, rng as usize % (i + 1));
        }
        for (i, choice) in orders.into_iter().enumerate() {
            m.set("pause", if i % 2 == 0 { "yes" } else { "no" })
                .unwrap();
            let pause = m.get("pause");
            if i % 3 == 0 {
                m.set("sid", "no").unwrap();
            } else {
                m.set("sid", "1").unwrap();
            }
            crate::subtitles::refresh(&m, &mut s);
            s.prefs.output = match choice {
                1 => Output::Anaglyph,
                2 => Output::Left,
                _ => Output::Original,
            };
            s.prefs.delivery = match choice {
                3 => Delivery::Anamorphic,
                4 => Delivery::FramePack,
                _ => Delivery::Native,
            };
            s.applied = None;
            player::apply(&m, &mut s).unwrap();
            s.features.invalidate();
            s.features.apply(&m, 0).unwrap();
            m.seek((i % 3) as f64, true).unwrap();
            let end = Instant::now() + Duration::from_millis(300);
            while Instant::now() < end {
                let e = m.wait(0.01);
                let (id, cookie) = (e.id, e.userdata);
                assert_ne!(id, 24, "Event queue overflow");
                if id == 5 && cookie & (1 << 63) != 0 {
                    m.seek_reply(cookie).unwrap();
                }
                if id == 17 {
                    s.restore_window(&m);
                }
            }
            s.restore_window(&m);
            assert_eq!(m.get("pause"), pause, "Mode switch changed pause");
            assert_eq!(
                crate::windows::display_geometry(crate::owner(&m)).unwrap()["client"],
                geometry,
                "Window changed: {file}/{choice}"
            );
            assert_eq!(m.get("video-params/primaries").as_deref(), Some("bt.709"));
            assert!(
                m.get("time-pos")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| v.is_finite() && v >= 0.0)
            );
            steps += 1;
        }
    }
    println!(
        "Seeded state walk: {steps} mode/track/pause/seek transitions across four input layouts"
    );
}

#[test]
#[ignore = "Run alone: process-wide Windows resource accounting"]
fn resident_engine_lifetimes_do_not_accumulate_windows_handles() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetProcessHandleCount(process: isize, count: *mut u32) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetGuiResources(process: *mut std::ffi::c_void, flags: u32) -> u32;
    }
    fn counts() -> [u32; 3] {
        let mut n = 0;
        assert_ne!(unsafe { GetProcessHandleCount(-1, &mut n) }, 0);
        [
            n,
            unsafe { GetGuiResources(-1isize as *mut std::ffi::c_void, 0) },
            unsafe { GetGuiResources(-1isize as *mut std::ffi::c_void, 1) },
        ]
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let _pin = (std::env::var_os("ENIGMAX_AUDIT_UNLOAD").is_none())
        .then(|| crate::windows::Library::load(&root.join("libmpv-2.dll")).unwrap());
    let mut samples = Vec::new();
    for pass in 0..10 {
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("autofit", "640x360").unwrap();
        if let Ok(vo) = std::env::var("ENIGMAX_AUDIT_VO") {
            m.option("vo", &vo).unwrap();
        }
        m.init().unwrap();
        m.command(&[
            "loadfile",
            root.join("verificacion/muestras/ass3d-black.mkv")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if m.wait(0.02).id == 8 {
                break;
            }
            assert!(Instant::now() < end);
        }
        pump(&mut m, Duration::from_millis(100));
        drop(m);
        std::thread::sleep(Duration::from_secs(1));
        let sample = counts();
        println!("Lifetime {pass}: kernel/GDI/USER={sample:?}");
        if pass >= 2 {
            samples.push(sample);
        }
    }
    for kind in 0..3 {
        let first = samples[0][kind];
        let last = samples.last().unwrap()[kind];
        assert!(
            last <= first + 8,
            "Repeated lifetime resource growth ({kind}): {samples:?}"
        );
    }
}
