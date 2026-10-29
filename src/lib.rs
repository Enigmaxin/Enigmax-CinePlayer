//! Geometry shared with a future Android frontend. No OS or decoder dependencies.
pub mod ass3d;
pub mod control_layout;
pub mod history;
pub mod media;
pub mod performance;
pub mod playback;
pub mod seek;
pub mod shader_store;
pub mod stereo;
pub mod update_cache;
pub mod update_policy;
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StereoMode {
    #[default]
    Original,
    FullSbs,
    HalfSbs,
    HalfOu,
}

impl StereoMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "Original / vídeo 2D",
            Self::FullSbs => "Full SBS → 2D",
            Self::HalfSbs => "HSBS → 2D",
            Self::HalfOu => "HOU → 2D",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presentation {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    /// Display aspect ratio of the entire packed frame, BEFORE cropping.
    pub packed_aspect: f64,
    pub eye_aspect: f64,
}

/// The explicit packing modes assume square pixels in the reconstructed eye.
/// `eye_aspect` permits manual correction for anamorphic or unusual sources.
/// Uneven packed dimensions are rejected: silently dropping a column/row changes AR.
pub fn presentation(
    width: u32,
    height: u32,
    mode: StereoMode,
    second_eye: bool,
    eye_aspect: Option<f64>,
) -> Result<Presentation, &'static str> {
    if width == 0 || height == 0 {
        return Err("El vídeo no tiene dimensiones válidas");
    }
    if eye_aspect.is_some_and(|v| !v.is_finite() || !(0.1..=10.0).contains(&v)) {
        return Err("Relación de aspecto fuera de rango");
    }
    let (w, h, sx, sy) = match mode {
        StereoMode::Original => (width, height, 1.0, 1.0),
        StereoMode::FullSbs | StereoMode::HalfSbs => {
            if !width.is_multiple_of(2) {
                return Err("La anchura SBS debe ser par");
            }
            (
                width / 2,
                height,
                if mode == StereoMode::HalfSbs {
                    2.0
                } else {
                    1.0
                },
                1.0,
            )
        }
        StereoMode::HalfOu => {
            if !height.is_multiple_of(2) {
                return Err("La altura HOU debe ser par");
            }
            (width, height / 2, 1.0, 2.0)
        }
    };
    let aspect = eye_aspect.unwrap_or(w as f64 * sx / (h as f64 * sy));
    Ok(Presentation {
        width: w,
        height: h,
        x: if second_eye && w != width { w } else { 0 },
        y: if second_eye && h != height { h } else { 0 },
        packed_aspect: aspect * (width as f64 / w as f64) / (height as f64 / h as f64),
        eye_aspect: aspect,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_packings_reconstruct_same_eye_aspect() {
        for (w, h, m) in [
            (3840, 1080, StereoMode::FullSbs),
            (1920, 1080, StereoMode::HalfSbs),
            (1920, 1080, StereoMode::HalfOu),
        ] {
            let p = presentation(w, h, m, false, None).unwrap();
            assert!((p.eye_aspect - 16.0 / 9.0).abs() < 1e-9);
            assert_eq!((p.x, p.y), (0, 0));
            let q = presentation(w, h, m, true, None).unwrap();
            assert_eq!(
                (q.x, q.y),
                if m == StereoMode::HalfOu {
                    (0, 540)
                } else {
                    (w / 2, 0)
                }
            );
        }
    }
    #[test]
    fn cropped_scope_preserves_geometry() {
        let p = presentation(3840, 800, StereoMode::FullSbs, false, None).unwrap();
        assert_eq!(p.eye_aspect, 2.4);
        assert_eq!(
            presentation(1920, 800, StereoMode::HalfSbs, false, None)
                .unwrap()
                .eye_aspect,
            2.4
        );
    }
    #[test]
    fn manual_anamorphic_correction_is_before_crop() {
        assert_eq!(
            presentation(1920, 1080, StereoMode::HalfSbs, false, Some(2.35))
                .unwrap()
                .packed_aspect,
            4.7
        );
        assert_eq!(
            presentation(1920, 1080, StereoMode::HalfOu, false, Some(2.35))
                .unwrap()
                .packed_aspect,
            1.175
        );
    }
    #[test]
    fn invalid_inputs_fail_without_guessing() {
        assert!(presentation(0, 1080, StereoMode::Original, false, None).is_err());
        assert!(presentation(1919, 1080, StereoMode::HalfSbs, false, None).is_err());
        assert!(presentation(1920, 1079, StereoMode::HalfOu, false, None).is_err());
        for a in [f64::NAN, f64::INFINITY, 0.0, -1.0] {
            assert!(presentation(1920, 1080, StereoMode::Original, false, Some(a)).is_err());
        }
    }
}
pub mod popup_geometry;
