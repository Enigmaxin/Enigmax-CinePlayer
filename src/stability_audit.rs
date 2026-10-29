//! Opt-in stress against the explicitly selected user file; originals read-only.
use crate::{
    configure,
    mpv::Mpv,
    player::{self, State},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[test]
#[ignore = "Requires ENIGMAX_TEST_STABILITY_FILE"]
fn real_file_repeated_reload_seek_and_shutdown() {
    let file = std::env::var("ENIGMAX_TEST_STABILITY_FILE").expect("Select an authorized file");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for session in 0..10 {
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut s = State::new(temp.path(), true);
        for repeat in 0..5 {
            println!("session={session} repeat={repeat} load begin");
            player::load(&m, &mut s, &file, None).unwrap();
            let until = Instant::now() + Duration::from_secs(15);
            loop {
                let event = m.wait(0.02);
                if event.id == 8 {
                    s.pending_load = false;
                }
                if event.id == 5 && event.userdata & (1_u64 << 63) != 0 {
                    let cookie = event.userdata;
                    m.seek_reply(cookie).unwrap();
                }
                if !s.pending_load && m.get("video-params/w").is_some() {
                    break;
                }
                assert!(Instant::now() < until, "Load timeout {session}/{repeat}");
            }
            player::apply(&m, &mut s).unwrap();
            assert_eq!(m.get("video-params/primaries").as_deref(), Some("bt.709"));
            assert_eq!(m.get("video-format").as_deref(), Some("hevc"));
            for t in [10.0, 60.0, 5.0] {
                m.seek(t, true).unwrap();
            }
            m.set("pause", "no").unwrap();
            for _ in 0..3 {
                let event = m.wait(0.03);
                if event.id == 5 && event.userdata & (1_u64 << 63) != 0 {
                    let cookie = event.userdata;
                    m.seek_reply(cookie).unwrap();
                }
            }
            m.set("pause", "yes").unwrap();
            println!("session={session} repeat={repeat} loaded, color/codec verified");
        }
        println!("session={session} shutdown begin");
        let now = Instant::now();
        m.reset_seeks();
        m.command_async(&["quit"], 0).unwrap();
        while m.wait(0.02).id != 1 {
            assert!(now.elapsed() < Duration::from_secs(12), "Shutdown timeout");
        }
        drop(m);
        assert!(now.elapsed() < Duration::from_secs(12));
        println!("session={session} destroyed in {:?}", now.elapsed());
    }
}

#[test]
#[ignore = "Requires ENIGMAX_TEST_STABILITY_FILE"]
fn real_file_quit_while_loading_and_replacing() {
    let file = std::env::var("ENIGMAX_TEST_STABILITY_FILE").expect("Select an authorized file");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for session in 0..30 {
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        for _ in 0..3 {
            m.command_async(&["loadfile", &file], 0).unwrap();
        }
        let now = Instant::now();
        for _ in 0..(session % 3) {
            m.wait(0.01);
        }
        m.command_async(&["quit"], 0).unwrap();
        while m.wait(0.02).id != 1 {
            assert!(
                now.elapsed() < Duration::from_secs(15),
                "Early quit timeout {session}"
            );
        }
        drop(m);
        assert!(now.elapsed() < Duration::from_secs(15));
        println!("early-close session {session}: {:?}", now.elapsed());
    }
}
