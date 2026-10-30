//! Live profile transitions and measured resources; no real device certification.
use crate::{
    configure,
    mpv::Mpv,
    player::{self, State},
};
use cineplayer_core::stereo::{Layout, Output, Quality};
use std::{
    path::Path,
    time::{Duration, Instant},
};
fn tick(m: &mut Mpv, s: &mut State) {
    let event = m.wait(0.01);
    let (id, cookie, error) = (event.id, event.userdata, event.error);
    assert!(
        !(id == 7
            && !event.data.is_null()
            && unsafe { (*event.data.cast::<crate::mpv::EndFile>()).reason } == 4),
        "Playback error"
    );
    if id == 5 && cookie & (1_u64 << 63) != 0 {
        m.seek_reply(cookie).unwrap();
    }
    if id == 5 {
        assert!(error >= 0, "Async control error {cookie}: {error}");
    }
    if id == 8 {
        s.pending_load = false;
    }
    let changed = (id == 8 || id == 22) && player::subtitle_event(m, s).unwrap();
    if id == 8 || id == 17 || changed {
        player::apply(m, s).unwrap();
    }
}
fn wait_for(m: &mut Mpv, s: &mut State, done: impl Fn(&Mpv, &State) -> bool) {
    let end = Instant::now() + Duration::from_secs(15);
    while !done(m, s) {
        tick(m, s);
        assert!(
            Instant::now() < end,
            "Quality state timeout: {}",
            s.pipeline
        );
    }
}
fn advance(m: &mut Mpv, s: &mut State, seconds: f64) {
    let end = Instant::now() + Duration::from_secs_f64(seconds);
    while Instant::now() < end {
        tick(m, s);
    }
}
fn setup(root: &Path) -> Mpv {
    let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    m.option("autofit", "960x540").unwrap();
    m.init().unwrap();
    m
}
fn number(m: &Mpv, key: &str) -> f64 {
    m.get(key)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}
#[test]
fn all_quality_transitions_keep_playback_tracks_color_and_transport() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = setup(root);
    let mut s = State::new(temp.path(), true);
    let mut cases = vec![
        ("pistas-qa", Layout::Mono, Output::Left, false),
        ("uhd_hdr10", Layout::Mono, Output::Left, false),
    ];
    for (file, layout) in [
        ("fullsbs", Layout::FullSbs),
        ("hsbs", Layout::HalfSbs),
        ("fullou", Layout::FullOu),
        ("hou", Layout::HalfOu),
    ] {
        for output in [Output::Original, Output::Left, Output::Anaglyph] {
            cases.push((file, layout, output, false));
        }
    }
    cases.extend([
        ("ass3d-fullsbs", Layout::FullSbs, Output::Anaglyph, true),
        ("ass3d-hou", Layout::HalfOu, Output::Anaglyph, true),
        ("depth-qa-pgs", Layout::HalfSbs, Output::Anaglyph, true),
    ]);
    let mut transitions = 0;
    let mut records = Vec::new();
    for (file, layout, output, subs) in cases {
        s.prefs.layout = layout;
        s.prefs.output = output;
        s.prefs.quality = Quality::Balanced;
        let file_path = root.join(format!("verificacion/muestras/quality-{file}.mkv"));
        player::load(&m, &mut s, file_path.to_str().unwrap(), None).unwrap();
        wait_for(&mut m, &mut s, |m, s| {
            !s.pending_load && m.get("video-params/w").is_some()
        });
        m.set("sid", if subs { "1" } else { "no" }).unwrap();
        advance(&mut m, &mut s, 0.12);
        player::apply(&m, &mut s).unwrap();
        let (aid, sid, gamma, prim, w, h) = (
            m.get("aid"),
            m.get("sid"),
            m.get("video-dec-params/gamma"),
            m.get("video-dec-params/primaries"),
            m.get("video-dec-params/w"),
            m.get("video-dec-params/h"),
        );
        let buffers = (m.get("demuxer-max-bytes"), m.get("demuxer-max-back-bytes"));
        m.set("pause", "no").unwrap();
        // Let the selected view finish its initial geometry reconfiguration;
        // subsequent size changes are then attributable to the quality action.
        advance(&mut m, &mut s, 0.4);
        for index in [1, 3, 2, 0, 2, 1, 0, 3, 0] {
            let before = number(&m, "time-pos");
            let before_drop = number(&m, "frame-drop-count");
            let window = (m.get("osd-width"), m.get("osd-height"));
            let start = Instant::now();
            player::action(&m, &mut s, 1050 + index).unwrap();
            let apply_ms = start.elapsed().as_secs_f64() * 1000.0;
            advance(&mut m, &mut s, 0.18);
            assert_eq!(
                (m.get("osd-width"), m.get("osd-height")),
                window,
                "Window resized on quality change: {file}/{index}"
            );
            records.push(serde_json::json!({"file":file,"output":format!("{output:?}"),"subtitles":subs,"quality":index,"apply_ms":apply_ms,"dropped":number(&m,"frame-drop-count")-before_drop,"avsync":m.get("avsync"),"hwdec":m.get("hwdec-current")}));
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "Slow switch {file}/{index}"
            );
            assert_eq!(m.get("pause").as_deref(), Some("no"));
            assert_eq!(m.get("aid"), aid);
            assert_eq!(m.get("sid"), sid);
            assert_eq!(m.get("video-dec-params/gamma"), gamma);
            assert_eq!(m.get("video-dec-params/primaries"), prim);
            assert_eq!(m.get("video-dec-params/w"), w);
            assert_eq!(m.get("video-dec-params/h"), h);
            assert!(
                number(&m, "time-pos") >= before - 0.15,
                "Position regressed {file}/{index}"
            );
            let q = Quality::ALL[index as usize];
            assert_eq!(s.prefs.quality, q);
            assert_eq!(
                m.get("scale").as_deref(),
                Some(match q {
                    Quality::High => "ewa_lanczossharp",
                    Quality::Low => "bilinear",
                    _ => "spline36",
                })
            );
            if q == Quality::Low {
                assert!(number(&m, "demuxer-max-bytes") <= 16.0 * 1024.0 * 1024.0);
                assert_eq!(m.get("demuxer-max-back-bytes").as_deref(), Some("0"));
                assert_eq!(m.get("hdr-compute-peak").as_deref(), Some("no"));
            } else {
                assert_eq!(
                    (m.get("demuxer-max-bytes"), m.get("demuxer-max-back-bytes")),
                    buffers
                );
                assert_eq!(m.get("hdr-compute-peak").as_deref(), Some("auto"));
            }
            let cpu = output == Output::Anaglyph && q == Quality::Compatibility && !subs;
            assert_eq!(
                m.get("vf").unwrap_or_default().contains("stereo3d"),
                cpu,
                "{file}/{q:?}: {}",
                s.pipeline
            );
            assert_eq!(m.get("hwdec").as_deref(), Some("auto"));
            transitions += 1;
        }
        println!("Quality transitions: {file} {output:?} subs={subs} OK");
    }
    println!("Validated {transitions} live quality changes");
    if let Ok(path) = std::env::var("ENIGMAX_QUALITY_TRANSITION_OUT") {
        std::fs::write(path, serde_json::to_vec_pretty(&records).unwrap()).unwrap();
    }
}
#[test]
fn compatibility_preserves_active_calibration_and_save_failure_rolls_back() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = setup(root);
    let mut s = State::new(temp.path(), true);
    s.prefs.layout = Layout::FullSbs;
    s.prefs.output = Output::Anaglyph;
    player::load(
        &m,
        &mut s,
        root.join("verificacion/muestras/quality-fullsbs.mkv")
            .to_str()
            .unwrap(),
        None,
    )
    .unwrap();
    wait_for(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    m.set("sid", "no").unwrap();
    for (gain, leak) in [(1.1, 0.0), (1.0, 0.05)] {
        s.prefs.left_gain = gain;
        s.prefs.crosstalk = leak;
        player::action(&m, &mut s, 1052).unwrap();
        advance(&mut m, &mut s, 0.2);
        assert_eq!(s.prefs.left_gain, gain);
        assert_eq!(s.prefs.crosstalk, leak);
        assert!(!m.get("vf").unwrap_or_default().contains("stereo3d"));
        assert!(s.pipeline.contains("calibración"));
    }
    s.prefs.left_gain = 1.0;
    s.prefs.crosstalk = 0.0;
    player::action(&m, &mut s, 1051).unwrap();
    std::fs::create_dir_all(temp.path().join("config/quality.json")).unwrap();
    s.automatic = true;
    assert!(player::action(&m, &mut s, 1053).is_err());
    assert_eq!(s.prefs.quality, Quality::High);
    assert_eq!(m.get("scale").as_deref(), Some("ewa_lanczossharp"));
    assert_eq!(m.get("demuxer-max-bytes").as_deref(), Some("67108864"));
    assert!(
        !temp
            .path()
            .join(format!("config/quality.{}.tmp", std::process::id()))
            .exists()
    );
}
#[test]
fn menu_choice_survives_in_a_fresh_application_state_process() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let m = setup(root);
    let mut state = State::new(temp.path(), false);
    for (index, quality) in Quality::ALL.iter().enumerate() {
        player::action(&m, &mut state, 1050 + index as u32).unwrap();
        assert_eq!(state.prefs.quality, *quality);
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "quality_audit::quality_restart_child",
                "--ignored",
                "--nocapture",
            ])
            .env("ENIGMAX_QUALITY_RESTART_DIR", temp.path())
            .env("ENIGMAX_QUALITY_RESTART_INDEX", index.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Restart failed: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        state.new_open();
        assert_eq!(state.prefs.quality, *quality);
    }
}
#[test]
#[ignore = "Child process used by quality persistence integration test"]
fn quality_restart_child() {
    let Some(dir) = std::env::var_os("ENIGMAX_QUALITY_RESTART_DIR") else {
        return;
    };
    let index = std::env::var("ENIGMAX_QUALITY_RESTART_INDEX")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(
        State::new(Path::new(&dir), false).prefs.quality,
        Quality::ALL[index]
    );
}
#[test]
#[ignore = "Requires ENIGMAX_TEST_MVC_ISO pointing to an authorized 3D ISO"]
fn mvc_quality_transitions_preserve_the_stream_and_restore_its_own_buffers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let mut m = setup(root);
    m.register_mvc(Box::new(crate::mvc::Registry::new(
        &root.join("runtime/cine_mvc.dll"),
    )))
    .unwrap();
    let mut s = State::new(temp.path(), true);
    s.prefs.layout = Layout::Mvc;
    s.prefs.output = Output::Anaglyph;
    player::load(
        &m,
        &mut s,
        &std::env::var("ENIGMAX_TEST_MVC_ISO").unwrap(),
        None,
    )
    .unwrap();
    wait_for(&mut m, &mut s, |m, s| {
        !s.pending_load && s.raw.is_some() && m.get("video-params/w").is_some()
    });
    m.set("sid", "no").unwrap();
    m.set("pause", "no").unwrap();
    advance(&mut m, &mut s, 0.4);
    let generation = s.generation;
    let path = m.get("path");
    let audio = m.get("aid");
    for index in [1, 3, 2, 0, 3, 1, 0] {
        player::action(&m, &mut s, 1050 + index).unwrap();
        advance(&mut m, &mut s, 0.25);
        assert!(s.raw.is_some());
        assert_eq!(s.generation, generation);
        assert_eq!(m.get("path"), path);
        assert_eq!(m.get("aid"), audio);
        assert_eq!(
            m.get("demuxer-max-bytes").as_deref(),
            Some(if index == 3 { "8388608" } else { "16777216" })
        );
    }
    println!("MVC: 7 live changes, no transport reload, 8/16 MiB restored correctly");
}
#[repr(C)]
#[derive(Default)]
struct Memory {
    cb: u32,
    faults: u32,
    values: [usize; 9],
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
    fn GetProcessTimes(
        p: *mut std::ffi::c_void,
        c: *mut u64,
        e: *mut u64,
        k: *mut u64,
        u: *mut u64,
    ) -> i32;
    fn K32GetProcessMemoryInfo(p: *mut std::ffi::c_void, m: *mut Memory, n: u32) -> i32;
}
fn resources() -> (f64, usize, usize) {
    let mut mem = Memory {
        cb: std::mem::size_of::<Memory>() as u32,
        ..Default::default()
    };
    let (mut c, mut e, mut k, mut u) = (0, 0, 0, 0);
    unsafe {
        let p = GetCurrentProcess();
        assert_ne!(GetProcessTimes(p, &mut c, &mut e, &mut k, &mut u), 0);
        assert_ne!(K32GetProcessMemoryInfo(p, &mut mem, mem.cb), 0);
    }
    ((k + u) as f64 / 10_000_000.0, mem.values[1], mem.values[8])
}
#[test]
#[ignore = "Run per-profile in a fresh process with ENIGMAX_QUALITY_BENCH_* env"]
fn measured_quality_resources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let temp = tempfile::tempdir().unwrap();
    let case = std::env::var("ENIGMAX_QUALITY_BENCH_CASE").unwrap();
    let index = std::env::var("ENIGMAX_QUALITY_BENCH_MODE")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let file = match case.as_str() {
        "sdr" | "sdr-upscale" => std::env::var("ENIGMAX_TEST_STABILITY_FILE").unwrap(),
        "hdr" | "hdr-native" => root
            .join("verificacion/muestras/quality-uhd_hdr10.mkv")
            .to_str()
            .unwrap()
            .into(),
        "anaglyph" => root
            .join("verificacion/muestras/quality-fullsbs.mkv")
            .to_str()
            .unwrap()
            .into(),
        _ => panic!("Unknown benchmark"),
    };
    let mut m = setup(root);
    if case == "sdr-upscale" {
        m.set("autofit", "2560x1440").unwrap();
    }
    if case == "hdr-native" {
        m.set("autofit", "3840x2160").unwrap();
    }
    let mut s = State::new(temp.path(), true);
    s.prefs.quality = Quality::ALL[index];
    if case == "anaglyph" {
        s.prefs.layout = Layout::FullSbs;
        s.prefs.output = Output::Anaglyph;
    }
    s.hdr_sdr = case.starts_with("hdr");
    player::load(&m, &mut s, &file, None).unwrap();
    wait_for(&mut m, &mut s, |m, s| {
        !s.pending_load && m.get("video-params/w").is_some()
    });
    m.set("sid", "no").unwrap();
    if case.starts_with("sdr") {
        m.seek(600.0, true).unwrap();
        wait_for(&mut m, &mut s, |m, _| {
            (number(m, "time-pos") - 600.0).abs() < 0.2
        });
    }
    player::apply(&m, &mut s).unwrap();
    m.set("pause", "no").unwrap();
    advance(&mut m, &mut s, 2.0);
    let before_cpu = resources().0;
    let before_dropped = number(&m, "frame-drop-count");
    let before_time = number(&m, "time-pos");
    let start = Instant::now();
    let (mut resident, mut private) = (0, 0);
    let mut max_sync: Option<f64> = None;
    while start.elapsed() < Duration::from_secs(4) {
        tick(&mut m, &mut s);
        let (_, ws, pr) = resources();
        resident = resident.max(ws);
        private = private.max(pr);
        if let Some(value) = m
            .get("avsync")
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite())
        {
            max_sync = Some(max_sync.unwrap_or(0.0).max(value.abs()));
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let cpu = (resources().0 - before_cpu) / elapsed;
    let passes: serde_json::Value =
        serde_json::from_str(&m.get("vo-passes").unwrap_or_default()).unwrap_or_default();
    let row = serde_json::json!({"case":case,"quality":index,"label":s.prefs.quality.label(),"elapsed":elapsed,"cpu_seconds_per_second":cpu,"working_mib":resident as f64/1048576.0,"private_mib":private as f64/1048576.0,"dropped":number(&m,"frame-drop-count")-before_dropped,"media_advanced":number(&m,"time-pos")-before_time,"max_avsync":max_sync,"osd_width":m.get("osd-width"),"osd_height":m.get("osd-height"),"hwdec":m.get("hwdec-current"),"renderer":m.get("current-vo"),"pipeline":s.pipeline,"vo_passes":passes});
    std::fs::write(
        std::env::var("ENIGMAX_QUALITY_BENCH_OUT").unwrap(),
        serde_json::to_vec_pretty(&row).unwrap(),
    )
    .unwrap();
    println!("{row}");
}
