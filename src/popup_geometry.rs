//! Shared projection and inverse pointer mapping for stereoscopic popups.
use crate::control_layout::{Plane, Projection, Rect};
pub struct Viewport {
    pub width: i32,
    pub height: i32,
    pub projection: Projection,
}
impl Viewport {
    pub fn new(width: i32, height: i32, projection: Projection) -> Option<Self> {
        ((160..=8192).contains(&width) && (100..=8192).contains(&height)).then_some(Self {
            width,
            height,
            projection,
        })
    }
    pub fn eyes(&self) -> Vec<Rect> {
        let (w, h) = (self.width, self.height);
        match self.projection {
            Projection::Sbs => vec![
                Rect {
                    x: 0,
                    y: 0,
                    w: w / 2,
                    h,
                },
                Rect {
                    x: w / 2,
                    y: 0,
                    w: w - w / 2,
                    h,
                },
            ],
            Projection::Ou => vec![
                Rect {
                    x: 0,
                    y: 0,
                    w,
                    h: h / 2,
                },
                Rect {
                    x: 0,
                    y: h / 2,
                    w,
                    h: h - h / 2,
                },
            ],
            Projection::FramePack => {
                let scale = (w as f64 / 1920.0).min(h as f64 / 2205.0);
                let ew = (1920.0 * scale).floor().max(1.0) as i32;
                let eh = (1080.0 * scale).floor().max(1.0) as i32;
                let x = (w - ew) / 2;
                let y = ((h as f64 - 2205.0 * scale) / 2.0).floor() as i32;
                vec![
                    Rect { x, y, w: ew, h: eh },
                    Rect {
                        x,
                        y: y + (1125.0 * scale).floor() as i32,
                        w: ew,
                        h: eh,
                    },
                ]
            }
            _ => vec![Rect { x: 0, y: 0, w, h }],
        }
    }
    fn disparity(&self, eye: usize) -> f64 {
        if matches!(self.projection, Projection::Mono | Projection::Anaglyph) {
            0.0
        } else {
            (self.width as f64 / 480.0).clamp(2.0, 8.0) * if eye == 0 { 1.0 } else { -1.0 }
        }
    }
    pub fn point(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        self.eyes()
            .iter()
            .enumerate()
            .find(|(_, r)| r.contains(x, y))
            .map(|(eye, r)| {
                (
                    (x - r.x as f64) * self.width as f64 / r.w as f64 - self.disparity(eye),
                    (y - r.y as f64) * self.height as f64 / r.h as f64,
                )
            })
    }
    pub fn project(&self, rect: Rect, pixels: &[u8]) -> Vec<Plane> {
        if rect.w <= 0 || rect.h <= 0 || pixels.len() != (rect.w * rect.h) as usize * 4 {
            return Vec::new();
        }
        self.eyes()
            .iter()
            .enumerate()
            .map(|(eye, r)| {
                let x = r.x
                    + ((rect.x as f64 + self.disparity(eye)) * r.w as f64 / self.width as f64)
                        .round() as i32;
                let y = r.y + (rect.y as f64 * r.h as f64 / self.height as f64).round() as i32;
                let w = (rect.w as f64 * r.w as f64 / self.width as f64)
                    .round()
                    .max(1.0) as i32;
                let h = (rect.h as f64 * r.h as f64 / self.height as f64)
                    .round()
                    .max(1.0) as i32;
                let mut output = vec![0; (w * h) as usize * 4];
                for yy in 0..h {
                    for xx in 0..w {
                        let src = ((yy * rect.h / h) * rect.w + xx * rect.w / w) as usize * 4;
                        let dst = (yy * w + xx) as usize * 4;
                        output[dst..dst + 4].copy_from_slice(&pixels[src..src + 4]);
                    }
                }
                Plane {
                    x,
                    y,
                    width: w,
                    height: h,
                    pixels: output,
                }
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn per_eye_popups_and_pointer_mapping_agree_in_all_stereo_transports() {
        for mode in [Projection::Sbs, Projection::Ou, Projection::FramePack] {
            for (w, h) in [
                (1920, 1080),
                (3840, 2160),
                (1919, 1079),
                (1280, 720),
                (1920, 2205),
            ] {
                let v = Viewport::new(w, h, mode).unwrap();
                let r = Rect {
                    x: w / 3,
                    y: h / 4,
                    w: w / 4,
                    h: h / 3,
                };
                let p = v.project(r, &vec![255; (r.w * r.h * 4) as usize]);
                assert_eq!(p.len(), 2);
                for plane in p {
                    let (x, y) = v
                        .point(
                            (plane.x + plane.width / 2) as f64,
                            (plane.y + plane.height / 2) as f64,
                        )
                        .unwrap();
                    assert!((x - (r.x + r.w / 2) as f64).abs() < 5.0);
                    assert!((y - (r.y + r.h / 2) as f64).abs() < 5.0);
                    assert!(r.contains(x, y));
                }
            }
        }
    }
    #[test]
    fn opposite_disparity_places_menus_in_front_without_affecting_the_video() {
        let v = Viewport::new(1920, 1080, Projection::Sbs).unwrap();
        let r = Rect {
            x: 600,
            y: 300,
            w: 500,
            h: 500,
        };
        let p = v.project(r, &vec![255; (r.w * r.h * 4) as usize]);
        assert!(p[0].x > r.x / 2);
        assert!(p[1].x < 960 + r.x / 2);
        assert!(v.point(-1.0, 0.0).is_none());
    }
}
