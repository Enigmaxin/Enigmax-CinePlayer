//! Decode actual synthetic compressed/lossless streams through the shipped libmpv.
//! A null sink proves decoding and format negotiation, not physical HDMI output.
use crate::{configure, mpv::Mpv, track_menus};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[test]
fn compressed_lossless_and_multichannel_audio_decode_and_track_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("verificacion/muestras/audio/matrix.json"))
            .expect("Run verificacion/tests/generate-audio.py first"),
    )
    .unwrap();
    let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
    configure(&m, true, root).unwrap();
    for (k, v) in [
        ("vo", "null"),
        ("ao", "null"),
        ("ao-null-untimed", "yes"),
        ("audio-channels", "auto"),
        ("pause", "no"),
    ] {
        m.option(k, v).unwrap();
    }
    m.init().unwrap();
    for case in matrix.as_array().unwrap() {
        m.reset_seeks();
        m.set("pause", "no").unwrap();
        let path = root
            .join("verificacion/muestras/audio")
            .join(format!("{}.mkv", case["name"].as_str().unwrap()));
        m.command(&["loadfile", path.to_str().unwrap()]).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        let mut loaded = false;
        let params = loop {
            let id = m.wait(0.01).id;
            if id == 8 {
                loaded = true;
            }
            assert!(Instant::now() < until, "Audio timeout: {case}");
            if !loaded {
                continue;
            }
            let params: serde_json::Value =
                serde_json::from_str(&m.get("audio-params").unwrap_or_default())
                    .unwrap_or_default();
            if m.get("audio-pts")
                .and_then(|v| v.parse::<f64>().ok())
                .is_some_and(|v| v >= 0.25)
                && params["samplerate"].is_number()
            {
                break params;
            }
        };
        assert_eq!(params["samplerate"], 48000, "{case}: {params}");
        assert_eq!(
            params["channel-count"], case["channels"],
            "{case}: {params}"
        );
        let (_, _, actions) = track_menus(&m).unwrap();
        assert!(
            actions
                .iter()
                .any(|(_, a)| matches!(a,crate::Action::Track(k,v) if k=="aid" && v=="1")),
            "Track snapshot lost audio: {case}"
        );
        m.set("pause", "yes").unwrap();
        m.seek(2.0, true).unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while m
            .get("time-pos")
            .and_then(|v| v.parse::<f64>().ok())
            .is_none_or(|v| (v - 2.0).abs() > 0.15)
        {
            let event = m.wait(0.01);
            let (id, cookie) = (event.id, event.userdata);
            if id == 5 && cookie & (1_u64 << 63) != 0 {
                m.seek_reply(cookie).unwrap();
            }
            assert!(Instant::now() < until, "Audio seek timeout: {case}");
        }
        println!(
            "{}: {} channels, 48000 Hz, decode/seek/menu OK",
            case["name"], case["channels"]
        );
    }
}
