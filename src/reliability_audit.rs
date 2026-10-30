use super::*;
use std::time::{Duration, Instant};
fn setup() -> (Mpv, State, tempfile::TempDir) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = tempfile::tempdir().unwrap();
    let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    crate::configure(&m, true, root).unwrap();
    m.init().unwrap();
    let mut m = m;
    m.register_mvc(Box::new(mvc::Registry::new(
        &root.join("runtime/cine_mvc.dll"),
    )))
    .unwrap();
    let s = State::new(dir.path(), false);
    (m, s, dir)
}
fn fixture(name: &str) -> String {
    std::path::absolute(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("verificacion/muestras")
            .join(name),
    )
    .unwrap()
    .to_str()
    .unwrap()
    .into()
}
fn pump(m: &mut Mpv, s: &mut State, until: impl Fn(&Mpv, &State) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let id = m.wait(0.01).id;
        if id == 8 {
            s.pending_load = false;
        }
        poll_loading(m, s).unwrap();
        s.restore_tracks(m).unwrap();
        s.restore_position(m).unwrap();
        apply(m, s).unwrap();
        if until(m, s) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out loading {:?}",
            s.source
        );
    }
}
#[test]
fn failed_async_iso_keeps_current_session_and_latest_request_wins() {
    let (mut m, mut s, dir) = setup();
    let original = fixture("pistas-qa.mkv");
    load(&m, &mut s, &original, None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    let source = s.source.clone();
    let generation = s.generation;
    let tracks = m.get("track-list");
    let path = m.get("path");
    let bad = dir.path().join("damaged.iso");
    std::fs::write(&bad, b"not an ISO").unwrap();
    let now = Instant::now();
    load(&m, &mut s, bad.to_str().unwrap(), None).unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
    assert!(loading(&s));
    let deadline = Instant::now() + Duration::from_secs(5);
    let error = loop {
        m.wait(0.01);
        if let Err(e) = poll_loading(&m, &mut s) {
            break e;
        }
        assert!(Instant::now() < deadline);
    };
    assert!(!error.is_empty());
    assert_eq!(s.source, source);
    assert_eq!(s.generation, generation);
    assert_eq!(m.get("path"), path);
    assert_eq!(m.get("track-list"), tracks);
    assert!(!loading(&s));
    load(&m, &mut s, bad.to_str().unwrap(), None).unwrap();
    let latest = fixture("title-qa.mkv");
    load(&m, &mut s, &latest, None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("path").as_deref() == Some(latest.as_str())
    });
    for _ in 0..30 {
        m.wait(0.01);
        poll_loading(&m, &mut s).unwrap();
    }
    assert_eq!(s.source, latest);
    assert_eq!(m.get("path").as_deref(), Some(latest.as_str()));
}
#[test]
fn mvc_metadata_and_non_square_pixels_reach_the_real_renderer() {
    let (mut m, mut s, _dir) = setup();
    load(&m, &mut s, &fixture("fullsbs.mkv"), None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    s.raw = Some(mvc::Info {
        metadata: mvc::Metadata {
            sar_num: 16,
            sar_den: 15,
            range: 2,
            matrix: 1,
            primaries: 9,
            transfer: 13,
            chroma: 1,
            ..Default::default()
        },
        ..Default::default()
    });
    s.prefs.output = Output::Left;
    s.applied = None;
    apply(&m, &mut s).unwrap();
    pump(&mut m, &mut s, |m, _| {
        m.get("video-out-params/primaries").as_deref() == Some("bt.2020")
            && m.get("video-out-params/gamma").as_deref() == Some("srgb")
    });
    assert_eq!(
        m.get("video-out-params/colorlevels").as_deref(),
        Some("full")
    );
    let aspect = m
        .get("video-aspect-override")
        .unwrap()
        .parse::<f64>()
        .unwrap();
    assert!((aspect - (3840.0 / 1080.0 * 16.0 / 15.0)).abs() < 1e-6);
    for transfer in [1, 4, 5, 7, 8, 13, 16, 18] {
        let tags = mvc::Metadata {
            matrix: 7,
            primaries: 7,
            transfer,
            range: 1,
            ..Default::default()
        }
        .filter();
        m.set("vf", &tags).unwrap();
    }
}
#[test]
fn failed_async_mvc_restores_the_previous_mode_without_replacing_the_session() {
    let (mut m, mut s, _dir) = setup();
    load(&m, &mut s, &fixture("pistas-qa.mkv"), None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    s.prefs.layout = Layout::Mvc;
    s.prefs.output = Output::Left;
    s.applied = None;
    apply(&m, &mut s).unwrap();
    let prefs = s.prefs.clone();
    let generation = s.generation;
    let source = s.source.clone();
    let video = m.get("path");
    action(&m, &mut s, 1200).unwrap();
    assert!(loading(&s));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        m.wait(0.01);
        if poll_loading(&m, &mut s).is_err() {
            break;
        }
        assert!(Instant::now() < deadline);
    }
    assert_eq!(s.prefs, prefs);
    assert_eq!(s.generation, generation);
    assert_eq!(s.source, source);
    assert_eq!(m.get("path"), video);
    assert!(s.raw.is_none());
}
#[test]
fn raw_mvc_transport_imports_subtitles_even_without_audio() {
    let (mut m, mut s, dir) = setup();
    let raw = dir.path().join("silent.yuv");
    std::fs::write(&raw, vec![128u8; 64 * 64 * 3 / 2 * 48]).unwrap();
    let info = mvc::Info {
        width: 64,
        height: 64,
        fps_num: 24,
        fps_den: 1,
        metadata: mvc::Metadata {
            has_sub: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    let opts = mvc_options(info, &fixture("ass3d-fullsbs.mkv"), &[]);
    m.command(&["loadfile", raw.to_str().unwrap(), "replace", "-1", &opts])
        .unwrap();
    s.pending_load = true;
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load
            && m.get("track-list")
                .is_some_and(|v| v.contains("\"type\":\"sub\""))
    });
    let tracks: serde_json::Value = serde_json::from_str(&m.get("track-list").unwrap()).unwrap();
    assert!(
        tracks
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["type"] == "sub" && r["codec"] == "ass")
    );
    assert!(
        !tracks
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["type"] == "audio")
    );
}
#[test]
fn failed_resume_unblocks_startup_and_update_shutdown_is_lifetime_safe() {
    let (mut m, mut s, _dir) = setup();
    load(&m, &mut s, &fixture("resume-qa.mkv"), None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("time-pos").is_some()
    });
    m.set("pause", "yes").unwrap();
    s.startup_choice = true;
    s.resume = Some("10000".into());
    s.resume_requested = true;
    s.resume_started = Some(Instant::now() - Duration::from_secs(13));
    s.restore_position(&m).unwrap();
    assert!(s.resume.is_none());
    assert!(s.startup_ready(&m));
    assert_eq!(m.get("pause").as_deref(), Some("yes"));
    let signal = m.shutdown_signal();
    let later = signal.clone();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        later.request();
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !m.shutdown_requested() {
        m.wait(0.2);
        assert!(Instant::now() < deadline);
    }
    worker.join().unwrap();
    crate::action(&m, &mut s, crate::Action::Id(55)).unwrap();
    assert!(s.closing);
    drop(s);
    drop(m);
    signal.request();
}
#[test]
fn replacing_media_invalidates_a_live_menu_snapshot() {
    let (mut m, mut s, dir) = setup();
    load(&m, &mut s, &fixture("pistas-qa.mkv"), None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    let script = dir.path().join("replace.lua");
    let target = serde_json::to_string(&fixture("fullsbs.mkv")).unwrap();
    std::fs::write(
        &script,
        format!("mp.add_timeout(0.15,function() mp.commandv('loadfile',{target},'replace') end)"),
    )
    .unwrap();
    m.command(&["load-script", script.to_str().unwrap()])
        .unwrap();
    let start = Instant::now();
    let result = crate::popup::show(
        &m,
        "Audio",
        vec![crate::popup::Entry::action(50, "Old track".into())],
    )
    .unwrap();
    assert_eq!(result, 0);
    assert!(start.elapsed() < Duration::from_secs(3));
}
#[test]
fn update_shutdown_closes_an_open_player_menu() {
    let (mut m, mut s, _dir) = setup();
    load(&m, &mut s, &fixture("pistas-qa.mkv"), None).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    let signal = m.shutdown_signal();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        signal.request();
    });
    let start = Instant::now();
    assert_eq!(
        crate::popup::show(
            &m,
            "Audio",
            vec![crate::popup::Entry::action(50, "Track".into())]
        )
        .unwrap(),
        0
    );
    worker.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(m.shutdown_requested());
}
#[test]
#[ignore = "Requires ENIGMAX_TEST_MVC_ISO for asynchronous ISO/MVC cancellation"]
fn async_iso_mvc_replace_and_quit_are_bounded() {
    let iso = std::env::var("ENIGMAX_TEST_MVC_ISO").unwrap();
    let (mut m, mut s, _dir) = setup();
    let start = Instant::now();
    load(&m, &mut s, &iso, None).unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    pump(&mut m, &mut s, |m, s| {
        !loading(s) && !s.pending_load && m.get("video-params/w").is_some()
    });
    let title = s.title;
    action(&m, &mut s, 1200).unwrap();
    pump(&mut m, &mut s, |m, s| {
        !loading(s) && !s.pending_load && s.raw.is_some() && m.get("video-params/w").is_some()
    });
    assert_eq!(s.title, title);
    for _ in 0..5 {
        action(&m, &mut s, 1210).unwrap();
        pump(&mut m, &mut s, |_, s| {
            !loading(s) && !s.pending_load && s.raw.is_none()
        });
        action(&m, &mut s, 1200).unwrap();
        cancel_loading(&mut s);
    }
    let start = Instant::now();
    crate::action(&m, &mut s, crate::Action::Id(55)).unwrap();
    drop(s);
    drop(m);
    assert!(start.elapsed() < Duration::from_secs(5));
}
