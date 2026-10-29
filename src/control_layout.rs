//! Screen-space controls: shared layout, hit testing and stereo projection.
use crate::stereo::Glasses;
pub fn depth_shift(_width: i32, _span: i32, _second: bool) -> i32 {
    // Flat HUD: identical normalized coordinates in both eyes.
    0
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Projection {
    #[default]
    Mono,
    Anaglyph,
    Sbs,
    Ou,
    FramePack,
}
impl Projection {
    pub fn code(self) -> &'static str {
        match self {
            Self::Mono => "mono",
            Self::Anaglyph => "anaglyph",
            Self::Sbs => "sbs",
            Self::Ou => "ou",
            Self::FramePack => "framepack",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Play,
    PreviousChapter,
    Chapters,
    NextChapter,
    Back,
    Forward,
    Mute,
    Stereo,
    Info,
    Swap,
    Audio,
    Subtitles,
    Fullscreen,
    Options,
    Volume,
    Seek,
}
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && x < (self.x + self.w) as f64
            && y >= self.y as f64
            && y < (self.y + self.h) as f64
    }
}
pub struct Layout {
    pub width: i32,
    pub height: i32,
    pub panel: i32,
    pub size: i32,
    pub regions: Vec<(Button, Rect)>,
    pub shelf: Rect,
}
impl Layout {
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if !(160..=8192).contains(&width) || !(100..=8192).contains(&height) {
            return None;
        }
        let panel = ((height as f64 * 0.1).round() as i32)
            .clamp(88, 160)
            .min(height);
        let size = (panel * 32 / 100).max(24).min(
            (width
                / if width >= 600 {
                    30
                } else if width >= 480 {
                    23
                } else {
                    18
                })
            .max(18),
        );
        let inset = (width / 60).clamp(8, 36);
        let padding = (panel / 7).min(width / 24).max(4);
        let margin = inset + padding;
        let gap = (size / 5).max(6);
        let row = panel * 56 / 100;
        let time_space = if width >= 600 { size * 3 } else { 0 };
        let seek = Rect {
            x: margin + time_space,
            y: panel * 24 / 100,
            w: width - 2 * (margin + time_space),
            h: (panel / 6).max(12),
        };
        let mut regions = vec![
            (Button::Seek, seek),
            (
                Button::Play,
                Rect {
                    x: margin,
                    y: row,
                    w: size,
                    h: size.min(panel - row - 3),
                },
            ),
        ];
        let mut x = margin + size + gap;
        for b in [
            Button::PreviousChapter,
            Button::Chapters,
            Button::NextChapter,
        ] {
            if width < 480 && b != Button::Chapters {
                continue;
            }
            regions.push((
                b,
                Rect {
                    x,
                    y: row,
                    w: size,
                    h: size.min(panel - row - 3),
                },
            ));
            x += size + gap;
        }
        let mut right = width - margin;
        if width >= 600 {
            for button in [Button::Back, Button::Forward] {
                regions.push((
                    button,
                    Rect {
                        x,
                        y: row,
                        w: size * 3 / 2,
                        h: size,
                    },
                ));
                x += size * 3 / 2 + gap;
            }
        }
        for (button, w) in [
            (Button::Fullscreen, size),
            (
                Button::Options,
                if width >= 800 { size * 3 / 2 } else { size },
            ),
            (
                Button::Subtitles,
                if width >= 800 && width >= size * 12 {
                    size * 2
                } else {
                    size
                },
            ),
            (
                Button::Audio,
                if width >= 800 && width >= size * 12 {
                    size * 2
                } else {
                    size
                },
            ),
            (Button::Stereo, size * 6 / 5),
            (Button::Swap, size),
            (Button::Info, size),
        ] {
            if right - w - gap < x {
                continue;
            }
            if (button == Button::Fullscreen && width < 360)
                || (button == Button::Stereo && width < 480)
            {
                continue;
            }
            right -= w;
            regions.push((
                button,
                Rect {
                    x: right,
                    y: row,
                    w,
                    h: size.min(panel - row - 3),
                },
            ));
            right -= gap;
        }
        if width >= 680 && right - x >= size + gap + 60 {
            regions.push((
                Button::Mute,
                Rect {
                    x,
                    y: row,
                    w: size,
                    h: size,
                },
            ));
            x += size + gap;
            let volume_width = (right - x - gap).clamp(40, 160);
            regions.push((
                Button::Volume,
                Rect {
                    x,
                    y: row,
                    w: volume_width,
                    h: size.min(panel - row - 3),
                },
            ));
        }
        Some(Self {
            width,
            height,
            panel,
            size,
            regions,
            shelf: Rect {
                x: inset,
                y: (panel / 12).max(4),
                w: width - inset * 2,
                h: panel - (panel / 12).max(4) * 2,
            },
        })
    }
    /// Fit the fixed canvas into the current window, then locate both HUDs.
    pub fn framepack_panels(&self) -> [Rect; 2] {
        let scale = (self.width as f64 / 1920.0).min(self.height as f64 / 2205.0);
        let width = (1920.0 * scale + 1e-7).floor().max(1.0) as i32;
        let x = ((self.width - width) / 2).max(0);
        let top = ((self.height as f64 - 2205.0 * scale) / 2.0)
            .max(0.0)
            .floor();
        let height = (self.panel as f64 * width as f64 / self.width as f64)
            .round()
            .max(1.0)
            .min((1080.0 * scale + 1e-7).floor().max(1.0)) as i32;
        [1080.0, 2205.0].map(|bottom| Rect {
            x,
            y: (top + bottom * scale + 1e-7).floor() as i32 - height,
            w: width,
            h: height,
        })
    }
    pub fn point(&self, p: Projection, x: f64, y: f64) -> Option<(f64, f64)> {
        if !x.is_finite()
            || !y.is_finite()
            || x < 0.0
            || y < 0.0
            || x >= self.width as f64
            || y >= self.height as f64
        {
            return None;
        }
        if p == Projection::FramePack {
            for (eye, r) in self.framepack_panels().into_iter().enumerate() {
                if r.contains(x, y) {
                    return Some((
                        (x - r.x as f64 - depth_shift(self.width, r.w, eye == 1) as f64)
                            * self.width as f64
                            / r.w as f64,
                        (y - r.y as f64) * self.panel as f64 / r.h as f64,
                    ));
                }
            }
            return None;
        }
        if p == Projection::Ou {
            let second = y >= (self.height / 2) as f64;
            let bh = (self.panel / 2).max(1);
            let base = if second { self.height } else { self.height / 2 } - bh;
            let yy = (y - base as f64) * self.panel as f64 / bh as f64;
            return (yy >= 0.0 && yy < self.panel as f64)
                .then_some((x - depth_shift(self.width, self.width, second) as f64, yy));
        }
        let (x, y) = match p {
            Projection::Sbs => {
                let left = self.width / 2;
                let (offset, span) = if x < left as f64 {
                    (0, left)
                } else {
                    (left, self.width - left)
                };
                (
                    (x - offset as f64 - depth_shift(self.width, span, offset != 0) as f64)
                        * self.width as f64
                        / span as f64,
                    y,
                )
            }
            Projection::Ou => (x, (y % (self.height as f64 / 2.0)) * 2.0),
            _ => (x, y),
        };
        let y = y - (self.height - self.panel) as f64;
        (y >= 0.0 && y < self.panel as f64).then_some((x, y))
    }
    pub fn hit(&self, p: Projection, x: f64, y: f64) -> Option<Button> {
        let (x, y) = self.point(p, x, y)?;
        self.regions
            .iter()
            .find(|(_, r)| r.contains(x, y))
            .map(|(b, _)| *b)
    }
    /// Once pressed, keep the original eye even outside its panel or across the seam.
    pub fn drag_fraction(
        &self,
        button: Button,
        p: Projection,
        second_eye: bool,
        x: f64,
    ) -> Option<f64> {
        if !x.is_finite() {
            return None;
        }
        let x = if p == Projection::FramePack {
            let r = self.framepack_panels()[0];
            (x - r.x as f64 - depth_shift(self.width, r.w, second_eye) as f64) * self.width as f64
                / r.w as f64
        } else if p == Projection::Sbs {
            let left = self.width / 2;
            let (offset, span) = if second_eye {
                (left, self.width - left)
            } else {
                (0, left)
            };
            (x - offset as f64 - depth_shift(self.width, span, second_eye) as f64)
                * self.width as f64
                / span as f64
        } else if p == Projection::Ou {
            x - depth_shift(self.width, self.width, second_eye) as f64
        } else {
            x
        };
        let r = self.regions.iter().find(|(b, _)| *b == button)?.1;
        Some(((x - r.x as f64) / r.w as f64).clamp(0.0, 1.0))
    }
    pub fn fraction(&self, button: Button, p: Projection, x: f64, y: f64) -> Option<f64> {
        let (x, _) = self.point(p, x, y)?;
        let r = self.regions.iter().find(|(b, _)| *b == button)?.1;
        Some(((x - r.x as f64) / r.w as f64).clamp(0.0, 1.0))
    }
}
pub struct Plane {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub pixels: Vec<u8>,
}
/// Flat controls, including transparent floating margins, projected per eye.
/// No video pixels are copied or filtered.
pub fn project(source: &[u8], layout: &Layout, p: Projection, g: Glasses) -> Vec<Plane> {
    let w = layout.width as usize;
    let h = layout.panel as usize;
    if source.len() != w * h * 4 {
        return Vec::new();
    }
    let make = |width: usize, height: usize, _eye: Option<bool>, _ana: bool| {
        if width == w && height == h {
            return source.to_vec();
        }
        let mut out = vec![0; 4 * width * height];
        for y in 0..height {
            if width == w {
                let src = (y * h / height) * w * 4;
                out[y * w * 4..(y + 1) * w * 4].copy_from_slice(&source[src..src + w * 4]);
                continue;
            }
            for x in 0..width {
                let src = ((y * h / height) * w + x * w / width) * 4;
                let dst = (y * width + x) * 4;
                out[dst..dst + 4].copy_from_slice(&source[src..src + 4]);
            }
        }
        out
    };
    let mut planes = match p {
        Projection::FramePack => layout
            .framepack_panels()
            .into_iter()
            .map(|r| Plane {
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
                pixels: make(r.w as usize, r.h as usize, None, false),
            })
            .collect(),
        Projection::Mono | Projection::Anaglyph => vec![Plane {
            x: 0,
            y: layout.height - layout.panel,
            width: layout.width,
            height: layout.panel,
            pixels: if p == Projection::Anaglyph {
                anaglyph_hud(source, layout.width, layout.panel, g)
            } else {
                make(w, h, None, false)
            },
        }],
        Projection::Sbs => {
            let left = w / 2;
            let right = w - left;
            vec![
                Plane {
                    x: 0,
                    y: layout.height - layout.panel,
                    width: left as i32,
                    height: layout.panel,
                    pixels: make(left, h, Some(false), false),
                },
                Plane {
                    x: left as i32,
                    y: layout.height - layout.panel,
                    width: right as i32,
                    height: layout.panel,
                    pixels: make(right, h, Some(true), false),
                },
            ]
        }
        Projection::Ou => {
            let height = (h / 2).max(1);
            vec![
                Plane {
                    x: 0,
                    y: layout.height / 2 - height as i32,
                    width: layout.width,
                    height: height as i32,
                    pixels: make(w, height, Some(false), false),
                },
                Plane {
                    x: 0,
                    y: layout.height - height as i32,
                    width: layout.width,
                    height: height as i32,
                    pixels: make(w, height, Some(true), false),
                },
            ]
        }
    };
    if matches!(p, Projection::Sbs | Projection::Ou | Projection::FramePack) {
        for (eye, plane) in planes.iter_mut().enumerate() {
            plane.x += depth_shift(layout.width, plane.width, eye == 1);
        }
    }
    planes
}
pub fn anaglyph_hud(source: &[u8], width: i32, height: i32, g: Glasses) -> Vec<u8> {
    if !(1..=8192).contains(&width)
        || !(1..=8192).contains(&height)
        || source.len() != (width * height) as usize * 4
    {
        return Vec::new();
    }
    let mut out = vec![0; source.len()];
    let shift = depth_shift(width, width, false);
    let (left, right) = match g {
        Glasses::RedCyan => ([false, false, true], [true, true, false]),
        Glasses::GreenMagenta => ([false, true, false], [true, false, true]),
        Glasses::AmberBlue => ([false, true, true], [true, false, false]),
        Glasses::RedBlue => ([false, false, true], [true, false, false]),
        Glasses::RedGreen => ([false, false, true], [false, true, false]),
    };
    for y in 0..height {
        for x in 0..width {
            let dst = (y * width + x) as usize * 4;
            for (second, mask) in [(false, left), (true, right)] {
                let xx = x + if second { shift } else { -shift };
                if !(0..width).contains(&xx) {
                    continue;
                }
                let src = (y * width + xx) as usize * 4;
                for c in 0..3 {
                    if mask[c] {
                        out[dst + c] = source[src + c];
                    }
                }
                out[dst + 3] = out[dst + 3].max(source[src + 3]);
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seek_and_volume_follow_the_actual_depth_shifted_planes_in_each_eye() {
        for (w, h) in [
            (640, 360),
            (1280, 720),
            (1919, 1079),
            (1920, 1080),
            (3840, 2160),
            (1920, 2205),
        ] {
            let l = Layout::new(w, h).unwrap();
            let pixels = vec![255; (w * l.panel * 4) as usize];
            for mode in [Projection::Sbs, Projection::Ou, Projection::FramePack] {
                for (eye, plane) in project(&pixels, &l, mode, Glasses::RedCyan)
                    .iter()
                    .enumerate()
                {
                    for button in [Button::Seek, Button::Volume] {
                        let Some((_, r)) = l.regions.iter().find(|(b, _)| *b == button) else {
                            continue;
                        };
                        for fraction in [0.0, 0.01, 0.5, 0.99, 1.0] {
                            let x = plane.x as f64
                                + (r.x as f64 + fraction * r.w as f64) * plane.width as f64
                                    / l.width as f64;
                            let y = plane.y as f64
                                + (r.y as f64 + r.h as f64 / 2.0) * plane.height as f64
                                    / l.panel as f64;
                            assert!(
                                (l.fraction(button, mode, x, y).unwrap() - fraction).abs() < 1e-9,
                                "{w}x{h} {mode:?} {button:?} eye {eye}"
                            );
                            assert!(
                                (l.drag_fraction(button, mode, eye == 1, x).unwrap() - fraction)
                                    .abs()
                                    < 1e-9
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn floating_margins_stay_transparent_in_every_projection() {
        let l = Layout::new(1920, 1080).unwrap();
        let mut source = vec![0; (l.width * l.panel * 4) as usize];
        for y in l.shelf.y + 8..l.shelf.y + 24 {
            for x in l.shelf.x + 8..l.shelf.x + 24 {
                let offset = (y * l.width + x) as usize * 4;
                source[offset..offset + 4].copy_from_slice(&[90, 100, 110, 255]);
            }
        }
        for mode in [
            Projection::Mono,
            Projection::Anaglyph,
            Projection::Sbs,
            Projection::Ou,
            Projection::FramePack,
        ] {
            for plane in project(&source, &l, mode, Glasses::RedCyan) {
                assert_eq!(&plane.pixels[..4], &[0, 0, 0, 0]);
                assert!(plane.pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
            }
        }
    }
    #[test]
    fn framepack_controls_are_inside_the_correct_eye_and_ignore_gap_and_margins() {
        for (w, h) in [
            (1920, 2205),
            (3840, 2160),
            (1280, 720),
            (800, 1200),
            (1919, 1079),
            (160, 8192),
            (8192, 100),
        ] {
            let l = Layout::new(w, h).unwrap();
            let panels = l.framepack_panels();
            assert!(panels[0].y + panels[0].h < panels[1].y);
            for (eye, panel) in panels.into_iter().enumerate() {
                assert!(
                    panel.x >= 0
                        && panel.y >= 0
                        && panel.x + panel.w <= w
                        && panel.y + panel.h <= h
                );
                for (button, r) in &l.regions {
                    let x = panel.x as f64
                        + (r.x as f64 + r.w as f64 / 2.0) * panel.w as f64 / w as f64
                        + depth_shift(w, panel.w, eye == 1) as f64;
                    let y = panel.y as f64
                        + (r.y as f64 + r.h as f64 / 2.0) * panel.h as f64 / l.panel as f64;
                    assert_eq!(l.hit(Projection::FramePack, x, y), Some(*button));
                    if *button == Button::Seek || *button == Button::Volume {
                        assert!(
                            (l.fraction(*button, Projection::FramePack, x, y).unwrap() - 0.5).abs()
                                < 1e-8
                        );
                        assert!(
                            (l.drag_fraction(*button, Projection::FramePack, eye == 1, x)
                                .unwrap()
                                - 0.5)
                                .abs()
                                < 1e-8
                        );
                    }
                }
            }
            assert!(
                l.point(Projection::FramePack, w as f64 / 2.0, h as f64 / 2.0)
                    .is_none()
            );
            if panels[0].x > 0 {
                assert!(
                    l.point(Projection::FramePack, 0.0, panels[0].y as f64)
                        .is_none()
                );
            }
            let pixels = vec![255; w as usize * l.panel as usize * 4];
            let planes = project(&pixels, &l, Projection::FramePack, Glasses::RedCyan);
            assert_eq!(planes.len(), 2);
            assert_eq!(planes[0].pixels, planes[1].pixels);
        }
    }
    #[test]
    fn dragging_uses_the_pressed_eye_and_clamps_without_wrapping() {
        for width in [640, 1280, 1919, 1920, 3840] {
            let l = Layout::new(width, 1080).unwrap();
            for button in [Button::Seek, Button::Volume] {
                let Some((_, r)) = l.regions.iter().find(|(b, _)| *b == button) else {
                    continue;
                };
                for second in [false, true] {
                    let split = width / 2;
                    let (off, span) = if second {
                        (split, width - split)
                    } else {
                        (0, split)
                    };
                    for fraction in [0.0, 0.1, 0.5, 0.9, 1.0] {
                        let x = off as f64
                            + (r.x as f64 + fraction * r.w as f64) * span as f64 / width as f64
                            + depth_shift(width, span, second) as f64;
                        assert!(
                            (l.drag_fraction(button, Projection::Sbs, second, x).unwrap()
                                - fraction)
                                .abs()
                                < 1e-9
                        );
                    }
                    assert_eq!(
                        l.drag_fraction(button, Projection::Sbs, second, off as f64 - 100.0),
                        Some(0.0)
                    );
                    assert_eq!(
                        l.drag_fraction(
                            button,
                            Projection::Sbs,
                            second,
                            (off + span) as f64 + 100.0
                        ),
                        Some(1.0)
                    );
                }
            }
        }
    }
    #[test]
    fn stereo_hits_match_both_eyes() {
        let l = Layout::new(1920, 1080).unwrap();
        let r = l
            .regions
            .iter()
            .find(|(b, _)| *b == Button::Play)
            .unwrap()
            .1;
        let x = (r.x + r.w / 2) as f64;
        let y = (l.height - l.panel + r.y + r.h / 2) as f64;
        assert_eq!(l.hit(Projection::Mono, x, y), Some(Button::Play));
        for xx in [x / 2.0, x / 2.0 + 960.0] {
            assert_eq!(l.hit(Projection::Sbs, xx, y), Some(Button::Play));
        }
        for yy in [y / 2.0, y / 2.0 + 540.0] {
            assert_eq!(l.hit(Projection::Ou, x, yy), Some(Button::Play));
        }
        assert!(l.hit(Projection::Sbs, -1.0, y).is_none());
    }
    #[test]
    fn chapter_buttons_have_distinct_regions_and_project_to_each_eye() {
        for (w, h) in [(640, 360), (1280, 720), (1920, 1080), (3840, 2160)] {
            let l = Layout::new(w, h).unwrap();
            for button in [
                Button::PreviousChapter,
                Button::Chapters,
                Button::NextChapter,
            ] {
                let r = l.regions.iter().find(|(b, _)| *b == button).unwrap().1;
                let x = (r.x + r.w / 2) as f64;
                let y = (h - l.panel + r.y + r.h / 2) as f64;
                assert_eq!(l.hit(Projection::Mono, x, y), Some(button));
                for eye in [0.0, 0.5] {
                    assert_eq!(
                        l.hit(Projection::Sbs, x / 2.0 + w as f64 * eye, y),
                        Some(button)
                    );
                }
                for eye in [0.0, 0.5] {
                    assert_eq!(
                        l.hit(Projection::Ou, x, y / 2.0 + h as f64 * eye),
                        Some(button)
                    );
                }
            }
        }
    }
    #[test]
    fn track_buttons_fit_and_hit_every_projection_without_overlapping() {
        for width in [160, 320, 480, 600, 800, 1000, 1920, 3840] {
            for height in [100, 720, 1080, 2160] {
                let l = Layout::new(width, height).unwrap();
                for (i, (b, r)) in l.regions.iter().enumerate() {
                    assert!(
                        r.x >= 0 && r.x + r.w <= width && r.y >= 0 && r.y + r.h <= l.panel,
                        "{width}x{height}: {b:?}"
                    );
                    for (other_button, other) in &l.regions[i + 1..] {
                        assert!(
                            r.x + r.w <= other.x
                                || other.x + other.w <= r.x
                                || r.y + r.h <= other.y
                                || other.y + other.h <= r.y,
                            "{width}x{height}: {b:?} overlaps {other_button:?}"
                        );
                    }
                }
                for button in [Button::Audio, Button::Subtitles, Button::Options] {
                    let r = l.regions.iter().find(|(b, _)| *b == button).unwrap().1;
                    let x = (r.x + r.w / 2) as f64;
                    let y = (height - l.panel + r.y + r.h / 2) as f64;
                    assert_eq!(l.hit(Projection::Mono, x, y), Some(button));
                    for xx in [x / 2.0, x / 2.0 + width as f64 / 2.0] {
                        assert_eq!(l.hit(Projection::Sbs, xx, y), Some(button));
                    }
                    for yy in [y / 2.0, y / 2.0 + height as f64 / 2.0] {
                        assert_eq!(l.hit(Projection::Ou, x, yy), Some(button));
                    }
                }
            }
        }
    }
    #[test]
    fn controls_are_flat_and_packed_output_is_aligned_per_eye() {
        let l = Layout::new(1920, 1080).unwrap();
        let mut src = vec![0; l.width as usize * l.panel as usize * 4];
        for p in src.as_chunks_mut::<4>().0 {
            p[3] = 255;
        }
        for y in 0..l.panel as usize {
            for x in 900..910 {
                src[(y * 1920 + x) * 4..(y * 1920 + x) * 4 + 4].fill(255);
            }
        }
        let p = project(&src, &l, Projection::Anaglyph, Glasses::RedCyan);
        assert_eq!(p.len(), 1);
        let a = &p[0].pixels;
        assert_eq!(a, &src);
        assert_eq!(
            project(&src, &l, Projection::Anaglyph, Glasses::AmberBlue)[0].pixels,
            src
        );
        let sbs = project(&src, &l, Projection::Sbs, Glasses::RedCyan);
        assert_eq!((sbs[0].x, sbs[1].x), (0, 960));
        assert_eq!(sbs[0].pixels, sbs[1].pixels);
        assert_eq!(
            project(&src, &l, Projection::Sbs, Glasses::RedCyan).len(),
            2
        );
        let ou = project(&src, &l, Projection::Ou, Glasses::RedCyan);
        assert_eq!(ou.len(), 2);
        assert!(ou[0].y < 540 && ou[1].y > 540);
        assert_eq!((ou[0].x, ou[1].x), (0, 0));
        assert_eq!(ou[0].pixels, ou[1].pixels);
    }
}
