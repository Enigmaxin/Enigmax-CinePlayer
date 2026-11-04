//! Runtime regressions: real DLLs, paused redraw, live transport and bounds.
use crate::{
    configure,
    mpv::Mpv,
    player::{self, State},
};
use cineplayer_core::stereo::{Delivery, Layout, Output, Quality};
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn pump(m: &mut Mpv, seconds: f64) {
    let end = Instant::now() + Duration::from_secs_f64(seconds);
    while Instant::now() < end {
        m.wait(0.01);
    }
}
fn engine(root: &Path) -> Mpv {
    let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    m.option("autofit", "960x540").unwrap();
    m.option("hwdec", "no").unwrap();
    m.init().unwrap();
    m
}
fn load(m: &mut Mpv, s: &mut State, root: &Path, name: &str) {
    player::load(
        m,
        s,
        root.join("verificacion/muestras")
            .join(name)
            .to_str()
            .unwrap(),
        None,
    )
    .unwrap();
    let end = Instant::now() + Duration::from_secs(12);
    loop {
        if m.wait(0.02).id == 8 {
            break;
        }
        assert!(Instant::now() < end);
    }
    s.pending_load = false;
    m.set("pause", "yes").unwrap();
    m.seek(0.5, true).unwrap();
    pump(m, 0.2);
}
fn identity(m: &Mpv) -> Vec<Option<String>> {
    [
        "path",
        "aid",
        "sid",
        "vf",
        "glsl-shaders",
        "current-vo",
        "pause",
        "video-params/w",
        "video-params/h",
        "audio-delay",
        "sub-delay",
    ]
    .iter()
    .map(|p| m.get(p))
    .collect()
}
#[test]
fn paused_parallax_changes_pixels_without_reloading_or_shader_variants() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = engine(root);
    let mut s = State::new(temp.path(), true);
    load(&mut m, &mut s, root, "depth-qa-pgs.mkv");
    m.set("sid", "1").unwrap();
    for (layout, delivery, output) in [
        (Layout::FullSbs, Delivery::Native, Output::Original),
        (Layout::HalfSbs, Delivery::Native, Output::Original),
        (Layout::FullOu, Delivery::Native, Output::Original),
        (Layout::HalfOu, Delivery::Native, Output::Original),
        (Layout::FullSbs, Delivery::Anamorphic, Output::Original),
        (Layout::HalfOu, Delivery::Anamorphic, Output::Original),
        (Layout::FullSbs, Delivery::FramePack, Output::Original),
        (Layout::HalfOu, Delivery::FramePack, Output::Original),
        (Layout::FullSbs, Delivery::Native, Output::Anaglyph),
        (Layout::HalfOu, Delivery::Native, Output::Anaglyph),
    ] {
        s.prefs.layout = layout;
        s.prefs.delivery = delivery;
        s.prefs.output = output;
        s.prefs.aspect = Some(16.0 / 9.0);
        s.applied = None;
        player::apply(&m, &mut s).unwrap();
        player::set_parallax(&m, &mut s, 0.0).unwrap();
        pump(&mut m, 0.15);
        let before = identity(&m);
        let generation = s.generation;
        let time = m.get("time-pos");
        let zero = m.screenshot_pixels_kind("window").unwrap().2;
        player::set_parallax(&m, &mut s, 0.02).unwrap();
        pump(&mut m, 0.12);
        let plus = m.screenshot_pixels_kind("window").unwrap().2;
        assert_ne!(
            plus, zero,
            "No paused redraw: {layout:?}/{delivery:?}/{output:?}"
        );
        player::set_parallax(&m, &mut s, -0.02).unwrap();
        pump(&mut m, 0.12);
        let minus = m.screenshot_pixels_kind("window").unwrap().2;
        assert_ne!(minus, plus, "Wrong signed parallax");
        player::set_parallax(&m, &mut s, 0.0).unwrap();
        pump(&mut m, 0.12);
        assert_eq!(
            m.screenshot_pixels_kind("window").unwrap().2,
            zero,
            "Zero did not restore pixels"
        );
        assert_eq!(identity(&m), before);
        assert_eq!(s.generation, generation);
        assert_eq!(m.get("time-pos"), time);
        println!(
            "Paused {layout:?}/{delivery:?}/{output:?}: +/−/zero, stable transport and shader"
        );
    }
    assert!(player::set_parallax(&m, &mut s, f32::NAN).is_err());
    assert!(player::set_parallax(&m, &mut s, 0.03).is_err());
    s.new_open();
    assert_eq!(s.parallax, 0.0);
}
#[test]
fn rapid_live_updates_preserve_tracks_clock_and_bounded_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = engine(root);
    let mut s = State::new(temp.path(), true);
    load(&mut m, &mut s, root, "quality-pistas-qa.mkv");
    s.prefs.layout = Layout::HalfSbs;
    s.prefs.output = Output::Anaglyph;
    m.set("sid", "no").unwrap();
    player::apply(&m, &mut s).unwrap();
    player::set_parallax(&m, &mut s, 0.0).unwrap();
    m.set("pause", "no").unwrap();
    pump(&mut m, 0.15);
    let before = identity(&m);
    let generation = s.generation;
    let start = m.get("time-pos").unwrap().parse::<f64>().unwrap();
    let mut ms = Vec::new();
    for i in 0..160 {
        let value = ((i * 17 % 41) as f32 - 20.0) / 1000.0;
        let now = Instant::now();
        player::set_parallax(&m, &mut s, value).unwrap();
        ms.push(now.elapsed().as_secs_f64() * 1000.0);
        pump(&mut m, 0.01);
    }
    assert_eq!(identity(&m), before);
    assert_eq!(s.generation, generation);
    let end = m.get("time-pos").unwrap().parse::<f64>().unwrap();
    assert!(end > start + 0.5, "Clock stalled");
    ms.sort_by(f64::total_cmp);
    println!(
        "160 live updates: median={:.3}ms p99={:.3}ms max={:.3}ms; clock {start:.3}->{end:.3}; drops={:?}",
        ms[80],
        ms[158],
        ms[159],
        m.get("frame-drop-count")
    );
    m.set("pause", "yes").unwrap();
    s.prefs.quality = Quality::Compatibility;
    s.applied = None;
    player::apply(&m, &mut s).unwrap();
    assert!(!player::parallax_supported(&m, &s));
    assert!(player::set_parallax(&m, &mut s, 0.01).is_err());
    assert_eq!(s.prefs.quality, Quality::Compatibility);
    assert_eq!(
        m.get("cine-video-parallax")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.0
    );
    load(&mut m, &mut s, root, "depth-qa-pgs.mkv");
    assert_eq!(s.parallax, 0.0);
}

#[test]
fn slider_and_back_keep_the_editor_live_in_each_output_projection() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = engine(root);
    let mut s = State::new(temp.path(), true);
    load(&mut m, &mut s, root, "depth-qa-pgs.mkv");
    m.set("sid", "1").unwrap();
    m.command(&[
        "load-script",
        root.join("verificacion/tests/stereo_popup.lua")
            .to_str()
            .unwrap(),
    ])
    .unwrap();
    pump(&mut m, 0.2);
    m.set("user-data/enigmax-toolbar-popup", "yes").unwrap();
    let mut controls = crate::controls::Controls::default();
    controls.ready();
    for (layout, delivery, output) in [
        (Layout::FullSbs, Delivery::Native, Output::Original),
        (Layout::HalfSbs, Delivery::Anamorphic, Output::Original),
        (Layout::HalfOu, Delivery::Native, Output::Original),
        (Layout::FullSbs, Delivery::FramePack, Output::Original),
        (Layout::FullSbs, Delivery::Native, Output::Anaglyph),
    ] {
        s.prefs.layout = layout;
        s.prefs.delivery = delivery;
        s.prefs.output = output;
        s.applied = None;
        player::apply(&m, &mut s).unwrap();
        controls.sync(&m, &s).unwrap();
        pump(&mut m, 0.15);
        let source = m.get("path");
        let generation = s.generation;
        let plan = serde_json::json!({"sequence":[16040,16000,16020,"@back-button",1550,16030,16020,"@escape"],"delay":0.06,"eye":1,"case":format!("{layout:?}/{delivery:?}/{output:?}")});
        m.set("user-data/enigmax-popup-test", &plan.to_string())
            .unwrap();
        player::action(&m, &mut s, 1550).unwrap();
        assert_eq!(s.parallax, 0.0);
        assert_eq!(s.generation, generation);
        assert_eq!(m.get("path"), source);
        assert_eq!(
            m.user_data("user-data/enigmax-popup-state").as_deref(),
            Some("")
        );
        assert_eq!(m.user_data("user-data/enigmax-popup-test-error"), None);
        pump(&mut m, 0.15); // allow Lua's close observer before reusing its client name
        println!(
            "{layout:?}/{delivery:?}/{output:?}: slider extremes, zero, Back and reopen from second eye"
        );
    }
    let entries = player::stereo_menu(&m, &s).unwrap().entries().unwrap();
    assert!(entries.iter().any(|e| e.id == 1550));
    assert!(!entries.iter().any(|e| e.label == "Avanzado"));
    assert!(entries.iter().any(|e| e.id == 1062));
}
