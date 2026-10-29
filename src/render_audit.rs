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
