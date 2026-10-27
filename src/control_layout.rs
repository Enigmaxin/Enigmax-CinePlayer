//! Screen-space controls: shared layout, hit testing and stereo projection.
use crate::stereo::Glasses;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Projection {
    #[default]
    Mono,
    Anaglyph,
    Sbs,
    Ou,
}
impl Projection {
    pub fn code(self) -> &'static str {
        match self {
            Self::Mono => "mono",
            Self::Anaglyph => "anaglyph",
            Self::Sbs => "sbs",
            Self::Ou => "ou",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Play,
    Back,
    Forward,
    Mute,
    Stereo,
    Audio,
    Subtitles,
    Fullscreen,
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
    pub time_x: i32,
    pub time_width: i32,
}
impl Layout {
    pub fn new(width: i32, height: i32) -> Option<Self> {
        if !(160..=8192).contains(&width) || !(100..=8192).contains(&height) {
            return None;
        }
        let panel = ((height as f64 * 0.1).round() as i32)
            .clamp(64, 160)
            .min(height);
        let size = (panel * 43 / 100).max(24).min((width / 10).max(18));
        let margin = (panel / 7).max(10);
        let gap = (size / 5).max(6);
        let row = panel * 58 / 100;
        let seek = Rect {
            x: margin,
            y: 8,
            w: width - 2 * margin,
            h: (panel / 4).max(16),
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
        if width >= 1000 && width >= size * 18 {
            for b in [Button::Back, Button::Forward] {
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
        }
        let time_x = x + gap;
        let mut right = width - margin;
        for (button, w) in [
            (Button::Fullscreen, size),
            (Button::Stereo, size * 6 / 5),
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
            (Button::Mute, size),
        ] {
            if (button == Button::Fullscreen && width < 360)
                || (button == Button::Stereo && width < 480)
                || (button == Button::Mute && width < 680)
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
        if right - time_x >= size * 7 + 110 {
            right -= 100;
            regions.push((
                Button::Volume,
                Rect {
                    x: right,
                    y: row,
                    w: 100,
                    h: size.min(panel - row - 3),
                },
            ));
            right -= gap;
        }
        Some(Self {
            width,
            height,
            panel,
            size,
            regions,
            time_x,
            time_width: (right - time_x - gap).max(0),
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
        let (x, y) = match p {
            Projection::Sbs => ((x % (self.width as f64 / 2.0)) * 2.0, y),
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
/// The panel is opaque: each color channel can represent its own eye without
/// incorrect cross-channel alpha attenuation. No video pixels are copied.
pub fn project(source: &[u8], layout: &Layout, p: Projection, g: Glasses) -> Vec<Plane> {
    let w = layout.width as usize;
    let h = layout.panel as usize;
    if source.len() != w * h * 4 {
        return Vec::new();
    }
    let depth = ((w as f64 * 0.001).round() as i32).clamp(1, 4);
    let sample = |x: i32, y: usize| -> &[u8] {
        let x = x.clamp(0, w as i32 - 1) as usize;
        &source[(y * w + x) * 4..(y * w + x + 1) * 4]
    };
    let make = |width: usize, height: usize, eye: Option<bool>, ana: bool| {
        let mut out = vec![0; 4 * width * height];
        for y in 0..height {
            let sy = y * h / height;
            for x in 0..width {
                let sx = (x * w / width) as i32;
                let dst = &mut out[(y * width + x) * 4..(y * width + x + 1) * 4];
                if ana {
                    let l = sample(sx - depth, sy);
                    let r = sample(sx + depth, sy);
                    let channels = match g {
                        Glasses::RedCyan => [false, false, true],
                        Glasses::GreenMagenta => [false, true, false],
                        Glasses::AmberBlue => [false, true, true],
                        Glasses::RedBlue => [false, false, true],
                        Glasses::RedGreen => [false, false, true],
                    };
                    for c in 0..3 {
                        dst[c] = if channels[c] { l[c] } else { r[c] };
                    }
                    if g == Glasses::RedBlue {
                        dst[1] = 0;
                    }
                    if g == Glasses::RedGreen {
                        dst[0] = 0;
                    }
                } else {
                    let shift = eye
                        .map(|right| if right { depth } else { -depth })
                        .unwrap_or(0);
                    dst[..3].copy_from_slice(&sample(sx + shift, sy)[..3]);
                }
                dst[3] = 255;
            }
        }
        out
    };
    match p {
        Projection::Mono | Projection::Anaglyph => vec![Plane {
            x: 0,
            y: layout.height - layout.panel,
            width: layout.width,
            height: layout.panel,
            pixels: make(w, h, None, p == Projection::Anaglyph),
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
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn track_buttons_fit_and_hit_every_projection_without_overlapping() {
        for width in [160, 320, 480, 600, 800, 1000, 1920, 3840] {
            for height in [100, 720, 1080, 2160] {
                let l = Layout::new(width, height).unwrap();
                for (i, (b, r)) in l.regions.iter().enumerate() {
                    assert!(
                        r.x >= 0 && r.x + r.w <= width && r.y >= 0 && r.y + r.h <= l.panel,
                        "{width}x{height}: {b:?}"
                    );
                    for (_, other) in &l.regions[i + 1..] {
                        assert!(
                            r.x + r.w <= other.x
                                || other.x + other.w <= r.x
                                || r.y + r.h <= other.y
                                || other.y + other.h <= r.y
                        );
                    }
                }
                for button in [Button::Audio, Button::Subtitles] {
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
    fn anaglyph_has_foreground_disparity_and_packed_output_is_per_eye() {
        let l = Layout::new(1920, 1080).unwrap();
        let mut src = vec![0; l.width as usize * l.panel as usize * 4];
        for y in 0..l.panel as usize {
            for x in 900..910 {
                src[(y * 1920 + x) * 4..(y * 1920 + x) * 4 + 4].fill(255);
            }
        }
        let p = project(&src, &l, Projection::Anaglyph, Glasses::RedCyan);
        assert_eq!(p.len(), 1);
        let a = &p[0].pixels;
        assert_eq!(a[902 * 4 + 2], 255);
        assert_eq!(a[898 * 4 + 1], 255);
        assert_eq!(a[898 * 4 + 2], 0);
        assert_eq!(a[911 * 4 + 1], 0);
        assert_eq!(
            project(&src, &l, Projection::Sbs, Glasses::RedCyan).len(),
            2
        );
        let ou = project(&src, &l, Projection::Ou, Glasses::RedCyan);
        assert_eq!(ou.len(), 2);
        assert!(ou[0].y < 540 && ou[1].y > 540);
    }
}
