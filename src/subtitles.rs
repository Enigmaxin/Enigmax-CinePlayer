use crate::{mpv::Mpv, player::State};
use cineplayer_core::ass3d::{Header, Mode, Packing};
use cineplayer_core::stereo::Layout;

#[derive(Default)]
pub struct Subtitles {
    track: String,
    extra: String,
    pub header: Option<Header>,
    pub detected: Option<Packing>,
}
impl Subtitles {
    pub fn requested(&self, mode: Mode) -> Option<Packing> {
        self.header.as_ref()?;
        match mode {
            Mode::Auto => self.detected,
            Mode::Off => None,
            Mode::Sbs => Some(Packing::Sbs),
            Mode::Ou => Some(Packing::Ou),
        }
    }
    pub fn active(&self, mode: Mode, layout: Layout) -> Option<Packing> {
        let packing = self.requested(mode)?;
        (layout.packed() && layout.horizontal() == (packing == Packing::Sbs)).then_some(packing)
    }
    pub fn status(&self, mode: Mode, layout: Layout) -> String {
        if mode == Mode::Off {
            return "ASS 3D desactivado por el usuario".into();
        }
        if self.header.is_none() {
            return "Sin pista ASS válida seleccionada".into();
        }
        let Some(packing) = self.requested(mode) else {
            return "ASS sin par estereoscópico confirmado".into();
        };
        if self.active(mode, layout).is_none() {
            return format!(
                "ASS {} identificado; selecciona el mismo formato de vídeo para transformarlo",
                packing.label()
            );
        }
        format!(
            "ASS 3D {} · {} · lienzo {:.0}×{:.0}",
            packing.label(),
            if mode == Mode::Auto {
                "duplicación detectada"
            } else {
                "selección manual"
            },
            self.header.as_ref().unwrap().canvas.width,
            self.header.as_ref().unwrap().canvas.height
        )
    }
}
/// Runs on actual property changes. It never scans the movie or extracts a
/// subtitle track; libass remains responsible for all rendering and animation.
pub fn refresh(m: &Mpv, s: &mut State) -> bool {
    let old_track = s.subtitles.track.clone();
    let old = s.subtitles.requested(s.prefs.ass_mode);
    let old_canvas = s.subtitles.header.as_ref().map(|h| h.canvas);
    let sid = m.get("sid").unwrap_or_default();
    let extra = m.get("sub-ass-extradata").unwrap_or_default();
    if sid != s.subtitles.track || extra != s.subtitles.extra {
        s.subtitles = Subtitles {
            track: sid,
            header: Header::parse(&extra),
            extra,
            detected: None,
        };
    }
    if s.subtitles.detected.is_none()
        && s.prefs.ass_mode == Mode::Auto
        && let Some(header) = &s.subtitles.header
    {
        s.subtitles.detected = header.detect(&m.get("sub-text/ass-full").unwrap_or_default());
    }
    let changed = old_track != s.subtitles.track
        || old != s.subtitles.requested(s.prefs.ass_mode)
        || old_canvas != s.subtitles.header.as_ref().map(|h| h.canvas);
    if changed {
        s.applied = None;
    }
    changed
}
pub fn eye_shader(horizontal: bool, second: bool) -> String {
    let axis = if horizontal { "x" } else { "y" };
    let size = if horizontal { "WIDTH" } else { "HEIGHT" };
    let dim = if horizontal { "w" } else { "h" };
    let offset = if second { 0.5 } else { 0.0 };
    format!(
        "//!HOOK MAIN\n//!BIND HOOKED\n//!DESC Enigmax eye selection ASS and video\n//!{size} HOOKED.{dim} 2 /\nvec4 hook(){{vec2 p=HOOKED_pos;p.{axis}=p.{axis}*0.5+{offset:.1};return HOOKED_tex(p);}}\n"
    )
}
