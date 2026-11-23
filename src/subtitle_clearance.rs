//! Temporary HUD clearance, independent from persisted subtitle preferences.
use crate::mpv::Mpv;
use cineplayer_core::control_layout::{Layout, Projection};

const BASE: &str = "user-data/enigmax-subtitle-position";
const CLEARANCE: &str = "user-data/enigmax-subtitle-clearance";

fn number(m: &Mpv, key: &str) -> Option<f64> {
    (if key.starts_with("user-data/") {
        m.user_data(key)
    } else {
        m.get(key)
    })
    .and_then(|s| s.parse().ok())
    .filter(|v: &f64| v.is_finite())
}

pub fn position(m: &Mpv) -> f64 {
    number(m, BASE)
        .or_else(|| number(m, "sub-pos"))
        .unwrap_or(100.0)
}

pub fn set_position(m: &Mpv, value: f64) -> Result<(), String> {
    if !value.is_finite() || !(0.0..=150.0).contains(&value) {
        return Err("Posición de subtítulos no válida".into());
    }
    m.set(BASE, &value.to_string())?;
    refresh(m)
}

fn reserve(layout: &Layout, projection: Projection) -> f64 {
    let fraction = if projection == Projection::FramePack {
        let panel = layout.framepack_panels()[0];
        let scale = (layout.width as f64 / 1920.0).min(layout.height as f64 / 2205.0);
        (panel.h as f64 + (8.0 * panel.w as f64 / layout.width as f64).max(2.0)) / (1080.0 * scale)
    } else {
        (layout.panel as f64 + 8.0) / layout.height as f64
    };
    (fraction * 100.0).clamp(0.0, 90.0)
}

pub fn controls(m: &Mpv, layout: Option<(&Layout, Projection)>) -> Result<(), String> {
    // Capture the user's position before the first temporary displacement.
    if number(m, BASE).is_none() {
        m.set(BASE, &position(m).to_string())?;
    }
    let value = layout.map_or(0.0, |(l, p)| reserve(l, p));
    if number(m, CLEARANCE).is_none_or(|previous| (previous - value).abs() > 0.001) {
        m.set(CLEARANCE, &value.to_string())?;
    }
    refresh(m)
}

pub fn refresh(m: &Mpv) -> Result<(), String> {
    let base = position(m);
    let supported = matches!(
        m.get("current-tracks/sub/codec").as_deref(),
        Some("subrip" | "srt" | "hdmv_pgs_subtitle" | "pgs")
    ) && m.get("sid").as_deref() != Some("no");
    let clearance = if supported {
        number(m, CLEARANCE).unwrap_or(0.0)
    } else {
        0.0
    };
    let effective = if clearance > 0.0 {
        base.min(100.0 - clearance)
    } else {
        base
    };
    if number(m, "sub-pos").is_none_or(|value| (value - effective).abs() > 0.001) {
        m.set("sub-pos", &effective.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        configure,
        player::{self, State},
    };
    use cineplayer_core::stereo::{Delivery, Glasses, Layout as Packing, Output};
    use std::{
        path::Path,
        time::{Duration, Instant},
    };

    fn pump(m: &mut Mpv) {
        let until = Instant::now() + Duration::from_millis(120);
        while Instant::now() < until {
            m.wait(0.01);
        }
    }

    #[test]
    fn subtitles_clear_each_eye_hud_restore_and_keep_authored_ass_and_preferences() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("pause", "yes").unwrap();
        m.option("autofit", "640x360").unwrap();
        m.init().unwrap();
        let mut s = State::new(temp.path(), true);
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
        while m.wait(0.02).id != 8 {
            assert!(Instant::now() < until);
        }
        s.pending_load = false;
        m.command(&["seek", "1", "absolute+exact"]).unwrap();
        pump(&mut m);
        let pgs = "1".to_owned();
        let mut tracks = vec![("pgs", pgs)];
        for ext in ["srt", "ass"] {
            m.command(&[
                "sub-add",
                root.join(format!("verificacion/muestras/depth-qa.{ext}"))
                    .to_str()
                    .unwrap(),
                "select",
            ])
            .unwrap();
            tracks.push((ext, m.get("sid").unwrap()));
        }
        let layouts = [
            (
                Packing::Mono,
                Output::Left,
                Delivery::Native,
                Projection::Mono,
            ),
            (
                Packing::FullSbs,
                Output::Original,
                Delivery::Native,
                Projection::Sbs,
            ),
            (
                Packing::HalfSbs,
                Output::Original,
                Delivery::Native,
                Projection::Sbs,
            ),
            (
                Packing::FullOu,
                Output::Original,
                Delivery::Native,
                Projection::Ou,
            ),
            (
                Packing::HalfOu,
                Output::Original,
                Delivery::Native,
                Projection::Ou,
            ),
            (
                Packing::FullSbs,
                Output::Original,
                Delivery::Anamorphic,
                Projection::Sbs,
            ),
            (
                Packing::FullSbs,
                Output::Anaglyph,
                Delivery::Native,
                Projection::Anaglyph,
            ),
            (
                Packing::FullSbs,
                Output::Original,
                Delivery::FramePack,
                Projection::FramePack,
            ),
        ];
        for (packing, output, delivery, projection) in layouts {
            s.prefs.layout = packing;
            s.prefs.output = output;
            s.prefs.delivery = delivery;
            for (codec, sid) in &tracks {
                controls(&m, None).unwrap();
                m.set("sid", sid).unwrap();
                s.features.invalidate();
                s.features.apply(&m, 0).unwrap();
                player::apply(&m, &mut s).unwrap();
                // The synthetic PGS has a single display event at t=0.
                // Re-read that event after each track switch before comparing pixels.
                m.command(&["seek", "1", "absolute+exact"]).unwrap();
                pump(&mut m);
                let source = (
                    m.get("vf"),
                    m.get("glsl-shaders"),
                    m.get("aid"),
                    m.get("time-pos"),
                );
                let until = Instant::now() + Duration::from_secs(5);
                let (w, h, before) = loop {
                    let image = m.screenshot_pixels_kind("window").unwrap();
                    if image
                        .2
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|p| p[..3].iter().any(|c| *c > 100))
                        .count()
                        > 20
                    {
                        break image;
                    }
                    assert!(
                        Instant::now() < until,
                        "Subtitle did not settle: {packing:?}/{codec}"
                    );
                    pump(&mut m);
                };
                let layout = Layout::new(w as i32, h as i32).unwrap();
                println!(
                    "Clearance {packing:?}/{delivery:?}/{codec} sid={sid} actual={:?} pos={:?} time={:?} size={w}x{h} bright={}",
                    m.get("current-tracks/sub/codec"),
                    m.get("sub-pos"),
                    m.get("time-pos"),
                    before
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|p| p[..3].iter().any(|c| *c > 100))
                        .count()
                );
                controls(&m, Some((&layout, projection))).unwrap();
                pump(&mut m);
                let (_, _, moved) = m.screenshot_pixels_kind("window").unwrap();
                println!(
                    "Effective {:?}, override {:?}, clearance {:?}",
                    m.get("sub-pos"),
                    m.get("sub-ass-override"),
                    m.get(CLEARANCE)
                );
                assert_eq!(position(&m), 100.0);
                assert_eq!(s.prefs.sub_pos, 100);
                assert_eq!(
                    source,
                    (
                        m.get("vf"),
                        m.get("glsl-shaders"),
                        m.get("aid"),
                        m.get("time-pos")
                    ),
                    "Reconfigured transport: {packing:?}/{codec}"
                );
                if *codec == "ass" {
                    assert!(
                        before == moved,
                        "ASS authoring moved: {packing:?}/{delivery:?}"
                    );
                } else {
                    assert!(
                        before != moved,
                        "No subtitle displacement: {packing:?}/{delivery:?}/{codec}"
                    );
                    let pixels = vec![0; (layout.width * layout.panel * 4) as usize];
                    for plane in cineplayer_core::control_layout::project(
                        &pixels,
                        &layout,
                        projection,
                        Glasses::RedCyan,
                    ) {
                        let overlap = moved
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .enumerate()
                            .filter(|(i, p)| {
                                let (x, y) = ((*i % w) as i32, (*i / w) as i32);
                                x >= plane.x
                                    && x < plane.x + plane.width
                                    && y >= plane.y
                                    && y < plane.y + plane.height
                                    && p[..3].iter().any(|c| *c > 100)
                            })
                            .count();
                        assert_eq!(
                            overlap, 0,
                            "Subtitle under HUD: {packing:?}/{delivery:?}/{codec}"
                        );
                    }
                    assert!(
                        moved
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .filter(|p| p[..3].iter().any(|c| *c > 100))
                            .count()
                            > 20,
                        "Subtitles disappeared"
                    );
                }
                controls(&m, None).unwrap();
                pump(&mut m);
                assert!(
                    m.screenshot_pixels_kind("window").unwrap().2 == before,
                    "Original position not restored: {packing:?}/{codec}"
                );
                assert_eq!(number(&m, "sub-pos"), Some(100.0));
            }
        }
        // A user edit during visibility is the base saved for the next session.
        m.set("sid", &tracks[1].1).unwrap();
        s.features.invalidate();
        s.features.apply(&m, 0).unwrap();
        let layout = Layout::new(1920, 1080).unwrap();
        controls(&m, Some((&layout, Projection::Sbs))).unwrap();
        set_position(&m, 95.0).unwrap();
        s.prefs.sub_pos = 95;
        s.automatic = true;
        s.save_visual().unwrap();
        assert!(number(&m, "sub-pos").unwrap() < 95.0);
        assert_eq!(State::new(temp.path(), false).prefs.sub_pos, 95);
        m.set("sid", &tracks[2].1).unwrap();
        refresh(&m).unwrap();
        assert_eq!(
            number(&m, "sub-pos"),
            Some(95.0),
            "Track switch must release temporary displacement for ASS"
        );
        m.set("sid", "no").unwrap();
        refresh(&m).unwrap();
        assert_eq!(number(&m, "sub-pos"), Some(95.0));
        controls(&m, None).unwrap();
        assert_eq!(position(&m), 95.0);
    }
    #[test]
    fn hud_clearance_is_bounded_and_consistent_across_eye_transports() {
        for (w, h) in [
            (160, 100),
            (641, 361),
            (1920, 1080),
            (3840, 2160),
            (8192, 8192),
        ] {
            let l = Layout::new(w, h).unwrap();
            let mono = reserve(&l, Projection::Mono);
            for p in [Projection::Anaglyph, Projection::Sbs, Projection::Ou] {
                assert_eq!(reserve(&l, p), mono);
            }
            let fp = reserve(&l, Projection::FramePack);
            assert!(fp.is_finite() && (0.0..=90.0).contains(&fp));
            assert!(mono.is_finite() && (0.0..=90.0).contains(&mono));
        }
    }

    #[test]
    fn repeated_live_clearance_changes_keep_clock_pipeline_and_bounded_position() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.option("autofit", "640x360").unwrap();
        m.init().unwrap();
        let mut s = State::new(temp.path(), true);
        s.prefs.layout = Packing::FullSbs;
        s.prefs.output = Output::Original;
        player::load(
            &m,
            &mut s,
            root.join("verificacion/muestras/quality-fullsbs.mkv")
                .to_str()
                .unwrap(),
            None,
        )
        .unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while m.wait(0.02).id != 8 {
            assert!(Instant::now() < until);
        }
        s.pending_load = false;
        m.command(&[
            "sub-add",
            root.join("verificacion/muestras/depth-qa.srt")
                .to_str()
                .unwrap(),
            "select",
        ])
        .unwrap();
        s.features.apply(&m, 0).unwrap();
        player::apply(&m, &mut s).unwrap();
        m.set("pause", "no").unwrap();
        pump(&mut m);
        let source = (
            m.get("vf"),
            m.get("glsl-shaders"),
            m.get("aid"),
            m.get("sid"),
        );
        let start = number(&m, "time-pos").unwrap();
        let begin = Instant::now();
        let mut last = start;
        for i in 0..30 {
            let l = Layout::new(if i % 3 == 0 { 3840 } else { 1920 }, 1080).unwrap();
            controls(&m, Some((&l, Projection::Sbs))).unwrap();
            // Neither repeated showing nor a resize may accumulate the offset.
            let value = number(&m, "sub-pos").unwrap();
            controls(&m, Some((&l, Projection::Sbs))).unwrap();
            assert_eq!(number(&m, "sub-pos"), Some(value));
            pump(&mut m);
            controls(&m, None).unwrap();
            assert_eq!(number(&m, "sub-pos"), Some(100.0));
            let now = number(&m, "time-pos").unwrap();
            assert!(now >= last - 0.05, "Playback moved backwards");
            last = now;
        }
        assert!(
            last - start >= begin.elapsed().as_secs_f64() * 0.75,
            "Playback did not keep pace"
        );
        assert_eq!(
            source,
            (
                m.get("vf"),
                m.get("glsl-shaders"),
                m.get("aid"),
                m.get("sid")
            )
        );
        assert_eq!(s.prefs.sub_pos, 100);
        println!(
            "30 show/hide/resize cycles, elapsed {:.3}s; video advanced {:.3}s",
            begin.elapsed().as_secs_f64(),
            last - start
        );
    }
}
