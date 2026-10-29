//! Bounded screen-space UI, drawn only while visible. No video readback.
use crate::{mpv::Mpv, player::State};
use cineplayer_core::{
    control_layout::{self, Button, Layout, Projection, Rect},
    stereo::{Glasses, Output},
};
use std::{ffi::c_void, ptr};
type Handle = *mut c_void;
#[link(name = "user32")]
unsafe extern "system" {
    fn DrawTextW(dc: Handle, text: *const u16, count: i32, rect: *mut [i32; 4], flags: u32) -> i32;
}
#[repr(C)]
struct BitmapHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bits: u16,
    compression: u32,
    image_size: u32,
    xppm: i32,
    yppm: i32,
    used: u32,
    important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapHeader,
    colors: [u32; 1],
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: Handle) -> Handle;
    fn DeleteDC(dc: Handle) -> i32;
    fn CreateDIBSection(
        dc: Handle,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: Handle,
        offset: u32,
    ) -> Handle;
    fn SelectObject(dc: Handle, object: Handle) -> Handle;
    fn DeleteObject(object: Handle) -> i32;
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        precision: u32,
        clip: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> Handle;
    fn SetBkMode(dc: Handle, mode: i32) -> i32;
    fn SetTextColor(dc: Handle, color: u32) -> u32;
    fn TextOutW(dc: Handle, x: i32, y: i32, text: *const u16, count: i32) -> i32;
    fn GdiFlush() -> i32;
}
pub(crate) struct Canvas {
    dc: Handle,
    bitmap: Handle,
    old_bitmap: Handle,
    font: Handle,
    old_font: Handle,
    bits: *mut u8,
    width: i32,
    height: i32,
}
impl Canvas {
    fn new(w: i32, h: i32) -> Result<Self, String> {
        Self::with_font(w, h, (h * 22 / 100).clamp(14, 36).min((w / 30).max(12)))
    }
    pub(crate) fn with_font(w: i32, h: i32, font_size: i32) -> Result<Self, String> {
        let dc = unsafe { CreateCompatibleDC(ptr::null_mut()) };
        if dc.is_null() {
            return Err("No se pudo crear el contexto de controles".into());
        }
        let info = BitmapInfo {
            header: BitmapHeader {
                size: 40,
                width: w,
                height: -h,
                planes: 1,
                bits: 32,
                compression: 0,
                image_size: 0,
                xppm: 0,
                yppm: 0,
                used: 0,
                important: 0,
            },
            colors: [0],
        };
        let mut bits = ptr::null_mut();
        let bitmap = unsafe { CreateDIBSection(dc, &info, 0, &mut bits, ptr::null_mut(), 0) };
        if bitmap.is_null() || bits.is_null() {
            unsafe { DeleteDC(dc) };
            return Err("No se pudo crear el panel de controles".into());
        }
        let face = "Segoe UI".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let font = unsafe {
            CreateFontW(
                -font_size,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                1,
                0,
                0,
                4,
                0,
                face.as_ptr(),
            )
        };
        if font.is_null() {
            unsafe {
                DeleteObject(bitmap);
                DeleteDC(dc)
            };
            return Err("No se pudo cargar la fuente de controles".into());
        }
        let old_bitmap = unsafe { SelectObject(dc, bitmap) };
        let old_font = unsafe { SelectObject(dc, font) };
        unsafe {
            SetBkMode(dc, 1);
            SetTextColor(dc, 0x00eeeeee)
        };
        Ok(Self {
            dc,
            bitmap,
            old_bitmap,
            font,
            old_font,
            bits: bits.cast(),
            width: w,
            height: h,
        })
    }
    pub(crate) fn pixels(&mut self) -> &[u8] {
        unsafe {
            GdiFlush();
            let pixels = std::slice::from_raw_parts_mut(
                self.bits,
                self.width as usize * self.height as usize * 4,
            );
            // GDI text clears alpha. Restore only painted pixels, retaining the
            // genuinely transparent margin outside the floating shelf.
            for p in pixels.as_chunks_mut::<4>().0 {
                if p[3] == 0 && p[..3] != [0, 0, 0] {
                    p[3] = 255;
                }
            }
            pixels
        }
    }
    fn clear(&mut self, layout: &Layout) {
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(
                self.bits,
                self.width as usize * self.height as usize * 4,
            )
        };
        pixels.fill(0);
        self.rounded(layout.shelf, (layout.panel / 10).max(6), [30, 28, 26, 255]);
    }
    pub(crate) fn rounded(&mut self, r: Rect, radius: i32, color: [u8; 4]) {
        let radius = radius.min(r.h / 2).min(r.w / 2).max(0);
        for dy in 0..r.h {
            let edge = if dy < radius {
                radius - dy - 1
            } else if dy >= r.h - radius {
                dy - (r.h - radius)
            } else {
                0
            };
            let cut = if edge > 0 {
                radius - ((radius * radius - edge * edge) as f64).sqrt() as i32
            } else {
                0
            };
            self.color_rect(
                Rect {
                    x: r.x + cut,
                    y: r.y + dy,
                    w: r.w - 2 * cut,
                    h: 1,
                },
                color,
            );
        }
    }
    pub(crate) fn rect(&mut self, r: Rect, value: u8) {
        self.color_rect(r, [value, value, value, 255]);
    }
    fn color_rect(&mut self, r: Rect, color: [u8; 4]) {
        let w = self.width;
        let h = self.height;
        let p = unsafe { std::slice::from_raw_parts_mut(self.bits, w as usize * h as usize * 4) };
        for y in r.y.max(0)..(r.y + r.h).min(h) {
            for x in r.x.max(0)..(r.x + r.w).min(w) {
                let i = (y * w + x) as usize * 4;
                p[i..i + 4].copy_from_slice(&color);
            }
        }
    }
    pub(crate) fn text(&self, x: i32, y: i32, text: &str) {
        let s = text.encode_utf16().collect::<Vec<_>>();
        unsafe { TextOutW(self.dc, x, y, s.as_ptr(), s.len() as i32) };
    }
    pub(crate) fn text_clipped(&self, x: i32, y: i32, w: i32, h: i32, text: &str) {
        if w <= 0 || h <= 0 || text.is_empty() {
            return;
        }
        let s = text.encode_utf16().collect::<Vec<_>>();
        let mut r = [x, y, (x + w).min(self.width), (y + h).min(self.height)];
        unsafe {
            DrawTextW(
                self.dc,
                s.as_ptr(),
                s.len() as i32,
                &mut r,
                0x20 | 0x800 | 0x8000,
            );
        }
    }
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            GdiFlush();
            SelectObject(self.dc, self.old_bitmap);
            SelectObject(self.dc, self.old_font);
            DeleteObject(self.bitmap);
            DeleteObject(self.font);
            DeleteDC(self.dc);
        }
    }
}
#[derive(Clone, Copy)]
struct Frame {
    w: i32,
    h: i32,
    x: f64,
    y: f64,
    time: f64,
    duration: f64,
    volume: f64,
    paused: bool,
    muted: bool,
    volume_feedback: i32,
}
impl Frame {
    fn parse(s: &str) -> Option<Self> {
        if s.len() > 512 {
            return None;
        }
        let a = s
            .split('|')
            .map(|v| v.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect::<Option<Vec<_>>>()?;
        if a.len() != 9 && a.len() != 10 {
            return None;
        }
        if a.get(9)
            .is_some_and(|v| ![-1.0, 0.0, 1.0, 2.0, 3.0].contains(v))
        {
            return None;
        }
        let (w, h) = (a[0] as i32, a[1] as i32);
        Layout::new(w, h)?;
        if !(-1.0..=w as f64).contains(&a[2])
            || !(-1.0..=h as f64).contains(&a[3])
            || !(0.0..=1e8).contains(&a[4])
            || !(0.0..=1e8).contains(&a[5])
            || !(0.0..=100.0).contains(&a[6])
        {
            return None;
        }
        Some(Self {
            w,
            h,
            x: a[2],
            y: a[3],
            time: a[4],
            duration: a[5],
            volume: a[6],
            paused: a[7] != 0.0,
            muted: a[8] != 0.0,
            volume_feedback: a.get(9).copied().unwrap_or(0.0) as i32,
        })
    }
}
pub enum Input {
    None,
    Menu,
    Options,
    Audio,
    Subtitles,
    Chapters,
    ChapterStep(i32),
    Info,
    Swap,
}
#[derive(Default)]
pub struct Controls {
    canvas: Option<Canvas>,
    mode: Projection,
    glasses: Glasses,
    ready: bool,
    config: String,
    last: String,
    drag: Option<Button>,
    drag_second_eye: bool,
    fraction: Option<f64>,
    rendered: bool,
    title: String,
}
impl Controls {
    fn idle_rects(m: &Mpv) -> Option<[Rect; 2]> {
        let w = m.get("osd-width")?.parse::<i32>().ok()?;
        let h = m.get("osd-height")?.parse::<i32>().ok()?;
        Layout::new(w, h)?;
        let bw = (w - 32).clamp(100, 360);
        let bh = (h / 10).clamp(32, 64);
        Some([
            Rect {
                x: (w - bw) / 2,
                y: (h - 2 * bh - 16) / 2,
                w: bw,
                h: bh,
            },
            Rect {
                x: (w - bw) / 2,
                y: (h - 2 * bh - 16) / 2 + bh + 16,
                w: bw,
                h: bh,
            },
        ])
    }
    pub fn idle_hit(&self, m: &Mpv, x: i32, y: i32) -> Option<bool> {
        if m.get("idle-active").as_deref() != Some("yes") {
            return None;
        }
        let rects = Self::idle_rects(m)?;
        if rects[0].contains(x as f64, y as f64) {
            Some(false)
        } else if rects[1].contains(x as f64, y as f64) {
            Some(true)
        } else {
            None
        }
    }
    pub fn idle_draw(&mut self, m: &Mpv, last: bool) -> Result<(), String> {
        if m.get("idle-active").as_deref() != Some("yes") {
            return Ok(());
        }
        let Some(rects) = Self::idle_rects(m) else {
            return Ok(());
        };
        let w = m
            .get("osd-width")
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(640);
        let mut c = Canvas::with_font(
            w,
            rects[1].y + rects[1].h - rects[0].y + 8,
            (rects[0].h / 3).clamp(14, 22),
        )?;
        let pixels =
            unsafe { std::slice::from_raw_parts_mut(c.bits, (c.width * c.height * 4) as usize) };
        pixels.fill(0);
        for (i, r) in rects.iter().enumerate() {
            let local = Rect {
                y: r.y - rects[0].y,
                ..*r
            };
            c.rounded(
                local,
                10,
                if i == 0 {
                    [51, 62, 57, 255]
                } else {
                    [33, 33, 33, 255]
                },
            );
            if i == 0 || last {
                c.text_clipped(
                    local.x + 16,
                    local.y + local.h / 4,
                    local.w - 32,
                    local.h * 3 / 4,
                    if i == 0 {
                        "Abrir vídeo"
                    } else {
                        "Continuar último vídeo"
                    },
                );
            } else {
                c.text_clipped(
                    local.x + 16,
                    local.y + local.h / 4,
                    local.w - 32,
                    local.h * 3 / 4,
                    "Sin vídeo anterior disponible",
                );
            }
        }
        m.command_named(
            "overlay-add",
            &[
                ("id", "63".into()),
                ("x", "0".into()),
                ("y", rects[0].y.to_string()),
                ("file", format!("&{}", c.pixels().as_ptr() as usize)),
                ("offset", "0".into()),
                ("fmt", "bgra".into()),
                ("w", c.width.to_string()),
                ("h", c.height.to_string()),
                ("stride", (c.width * 4).to_string()),
            ],
        )
    }
    pub fn rendered(&self) -> bool {
        self.rendered
    }
    pub fn ready(&mut self) {
        self.ready = true;
        self.config.clear();
    }
    pub fn sync(&mut self, m: &Mpv, s: &State) -> Result<(), String> {
        if self.title != s.display_title {
            self.title = s.display_title.clone();
            self.last.clear();
        }
        if !self.ready || s.pending_load {
            return Ok(());
        }
        let layout = s.effective(m);
        self.mode = if !layout.packed() {
            Projection::Mono
        } else {
            match s.prefs.output {
                Output::Anaglyph => Projection::Anaglyph,
                Output::Original => {
                    if s.prefs.delivery == cineplayer_core::stereo::Delivery::FramePack {
                        Projection::FramePack
                    } else if s.prefs.delivery != cineplayer_core::stereo::Delivery::Native
                        || layout.horizontal()
                    {
                        Projection::Sbs
                    } else {
                        Projection::Ou
                    }
                }
                _ => Projection::Mono,
            }
        };
        self.glasses = s.prefs.glasses;
        let config = format!("{}:{}", self.mode.code(), self.glasses as u8);
        if self.config != config {
            m.command(&[
                "script-message-to",
                "enigmax_controls",
                "configure",
                self.mode.code(),
            ])?;
            self.config = config;
            m.set("user-data/enigmax-projection", self.mode.code())?;
            m.set(
                "user-data/enigmax-glasses",
                &(self.glasses as u8).to_string(),
            )?;
            self.drag = None;
            self.fraction = None;
            self.last.clear();
        }
        Ok(())
    }
    pub fn hide(&mut self, m: &Mpv) -> Result<(), String> {
        self.rendered = false;
        for id in [61, 62] {
            m.command(&["overlay-remove", &id.to_string()])?;
        }
        self.last.clear();
        self.drag = None;
        self.fraction = None;
        Ok(())
    }
    pub fn draw(&mut self, m: &Mpv, body: &str) -> Result<(), String> {
        let f = Frame::parse(body).ok_or("Estado de controles no válido")?;
        let l = Layout::new(f.w, f.h).ok_or("Tamaño de controles no válido")?;
        let hover = l.hit(self.mode, f.x, f.y);
        let progress = self
            .fraction
            .filter(|_| self.drag == Some(Button::Seek))
            .unwrap_or_else(|| {
                if f.duration > 0.0 {
                    f.time / f.duration
                } else {
                    0.0
                }
            })
            .clamp(0.0, 1.0);
        let shown = self
            .fraction
            .filter(|_| self.drag == Some(Button::Seek))
            .map(|p| p * f.duration)
            .unwrap_or(f.time);
        let volume = self
            .fraction
            .filter(|_| self.drag == Some(Button::Volume))
            .unwrap_or(f.volume / 100.0);
        let seek_width = l
            .regions
            .iter()
            .find(|(b, _)| *b == Button::Seek)
            .map_or(1, |(_, r)| r.w);
        let key = format!(
            "{}x{}/{:?}/{:?}/{:?}/{}/{}/{}/{}/{}/{}/{}",
            f.w,
            f.h,
            self.mode,
            self.glasses,
            hover,
            (progress * seek_width as f64) as i32,
            shown as u64,
            f.duration as u64,
            (volume * 1000.0) as u32,
            f.paused,
            f.muted,
            f.volume_feedback
        );
        if key == self.last {
            return Ok(());
        }
        if self
            .canvas
            .as_ref()
            .is_none_or(|c| c.width != l.width || c.height != l.panel)
        {
            self.canvas = Some(Canvas::new(l.width, l.panel)?);
        }
        let c = self.canvas.as_mut().ok_or("No hay lienzo de controles")?;
        c.clear(&l);
        c.text_clipped(
            l.shelf.x + 10,
            0,
            l.shelf.w - 20,
            l.panel * 24 / 100,
            &self.title,
        );
        let mut texts = Vec::new();
        for (button, r) in &l.regions {
            let bright = if hover == Some(*button) { 255 } else { 195 };
            if hover == Some(*button) && !matches!(button, Button::Seek | Button::Volume) {
                c.rounded(*r, (l.size / 6).max(4), [52, 49, 47, 255]);
            }
            let cy = r.y + r.h / 2;
            match button {
                Button::Seek => {
                    c.rect(
                        Rect {
                            x: r.x,
                            y: cy - 2,
                            w: r.w,
                            h: 4,
                        },
                        65,
                    );
                    let filled = (progress * r.w as f64) as i32;
                    c.color_rect(
                        Rect {
                            x: r.x,
                            y: cy - 2,
                            w: filled,
                            h: 4,
                        },
                        [188, 217, 147, 255],
                    );
                    c.rect(
                        Rect {
                            x: r.x + filled - 3,
                            y: cy - 5,
                            w: 6,
                            h: 10,
                        },
                        bright,
                    );
                }
                Button::Volume => {
                    c.rect(
                        Rect {
                            x: r.x,
                            y: cy - 2,
                            w: r.w,
                            h: 4,
                        },
                        65,
                    );
                    c.rect(
                        Rect {
                            x: r.x,
                            y: cy - 2,
                            w: (r.w as f64
                                * self
                                    .fraction
                                    .filter(|_| self.drag == Some(Button::Volume))
                                    .unwrap_or(f.volume / 100.0))
                                as i32,
                            h: 4,
                        },
                        bright,
                    );
                }
                Button::Play => {
                    let x = r.x + r.w / 3;
                    let d = (r.h / 4).max(5);
                    if f.paused {
                        for dy in -d..=d {
                            c.rect(
                                Rect {
                                    x,
                                    y: cy + dy,
                                    w: (d - (dy.abs() / 2)).max(1),
                                    h: 1,
                                },
                                bright,
                            );
                        }
                    } else {
                        for dx in [0, d] {
                            c.rect(
                                Rect {
                                    x: x + dx,
                                    y: cy - d,
                                    w: (d / 2).max(3),
                                    h: d * 2,
                                },
                                bright,
                            );
                        }
                    }
                }
                Button::PreviousChapter | Button::NextChapter => {
                    let d = (r.h / 4).max(3);
                    let cx = r.x + r.w / 2;
                    let right = *button == Button::NextChapter;
                    let tip = if right { cx + d / 2 } else { cx - d / 2 };
                    let base = if right { cx - d / 2 } else { cx + d / 2 };
                    for dy in -d..=d {
                        let length = (d - dy.abs()).max(1);
                        c.rect(
                            Rect {
                                x: if right { base } else { tip + d - length },
                                y: cy + dy,
                                w: length,
                                h: 1,
                            },
                            bright,
                        );
                    }
                    c.rect(
                        Rect {
                            x: if right { cx + d } else { cx - d - 2 },
                            y: cy - d,
                            w: 2,
                            h: 2 * d,
                        },
                        bright,
                    );
                }
                Button::Chapters => {
                    let x = r.x + r.w / 4;
                    for dy in [-r.h / 5, 0, r.h / 5] {
                        c.rect(
                            Rect {
                                x,
                                y: cy + dy,
                                w: r.w / 2,
                                h: 2,
                            },
                            bright,
                        );
                    }
                }
                Button::Back => texts.push((r.x, cy - l.panel / 10, "−10".into())),
                Button::Forward => texts.push((r.x, cy - l.panel / 10, "+10".into())),
                Button::Stereo => texts.push((r.x + r.w / 5, cy - l.panel / 10, "3D".into())),
                Button::Info => texts.push((r.x + r.w / 4, cy - l.panel / 10, "ⓘ".into())),
                Button::Swap => texts.push((r.x + r.w / 5, cy - l.panel / 10, "⇄".into())),
                Button::Audio => texts.push((
                    r.x + r.w / 8,
                    cy - l.panel / 10,
                    if r.w >= l.size * 2 { "Audio" } else { "A" }.into(),
                )),
                Button::Subtitles => texts.push((
                    r.x + r.w / 8,
                    cy - l.panel / 10,
                    if r.w >= l.size * 2 { "Subs" } else { "S" }.into(),
                )),
                Button::Options => texts.push((r.x + r.w / 5, cy - l.panel / 10, "⋯".into())),
                Button::Mute => {
                    let x = r.x + r.w / 3;
                    c.rect(
                        Rect {
                            x,
                            y: cy - 4,
                            w: 6,
                            h: 9,
                        },
                        bright,
                    );
                    for dx in 0..7 {
                        c.rect(
                            Rect {
                                x: x + 6 + dx,
                                y: cy - 4 - dx / 2,
                                w: 1,
                                h: 9 + dx,
                            },
                            bright,
                        );
                    }
                    if f.muted {
                        texts.push((r.x + r.w * 2 / 3, cy - l.panel / 10, "×".into()));
                    }
                }
                Button::Fullscreen => {
                    let x = r.x + r.w / 4;
                    let w = r.w / 2;
                    let h = r.h / 2;
                    let y = cy - h / 2;
                    for (dx, dy) in [(0, 0), (w - 5, 0), (0, h - 2), (w - 5, h - 2)] {
                        c.rect(
                            Rect {
                                x: x + dx,
                                y: y + dy,
                                w: 6,
                                h: 2,
                            },
                            bright,
                        );
                    }
                    for (dx, dy) in [(0, 0), (w - 2, 0), (0, h - 6), (w - 2, h - 6)] {
                        c.rect(
                            Rect {
                                x: x + dx,
                                y: y + dy,
                                w: 2,
                                h: 6,
                            },
                            bright,
                        );
                    }
                }
            }
        }
        if l.width >= 600 {
            let seek = l
                .regions
                .iter()
                .find(|(b, _)| *b == Button::Seek)
                .unwrap()
                .1;
            let y = seek.y + seek.h / 2 - l.panel / 10;
            texts.push((l.shelf.x + (l.panel / 7), y, time(shown)));
            texts.push((seek.x + seek.w + l.size / 3, y, time(f.duration)));
        }
        if let Some(
            button @ (Button::Audio
            | Button::Subtitles
            | Button::PreviousChapter
            | Button::Chapters
            | Button::NextChapter
            | Button::Info
            | Button::Swap),
        ) = hover
        {
            // A descriptive hint also explains compact A/S buttons in small windows.
            texts.push((
                l.width / 2 - l.size * 3,
                l.panel * 42 / 100,
                match button {
                    Button::Info => "Información de audio y vídeo",
                    Button::Swap => "Intercambiar ojos izquierdo / derecho",
                    Button::Audio => "Pista de audio",
                    Button::Subtitles => "Subtítulos",
                    Button::PreviousChapter => "Capítulo anterior",
                    Button::NextChapter => "Capítulo siguiente",
                    _ => "Ir a un capítulo",
                }
                .into(),
            ));
        }
        if f.volume_feedback != 0 {
            let label = match f.volume_feedback {
                2 => "Sonido silenciado".to_owned(),
                3 => format!("Sonido activado · {:.0} %", f.volume),
                1 => format!("Volumen ↑ {:.0} %", f.volume),
                _ => format!("Volumen ↓ {:.0} %", f.volume),
            };
            let font = (l.panel * 22 / 100)
                .clamp(14, 36)
                .min((l.width / 18).max(12));
            let text_width = label.chars().count() as i32 * font / 2;
            texts.push((
                ((l.width - text_width) / 2).max(4),
                l.panel * 42 / 100,
                label,
            ));
        }
        for (x, y, text) in texts {
            c.text(x, y, &text);
        }
        let planes = control_layout::project(c.pixels(), &l, self.mode, self.glasses);
        let count = planes.len();
        for (i, p) in planes.iter().enumerate() {
            m.command_named(
                "overlay-add",
                &[
                    ("id", (61 + i).to_string()),
                    ("x", p.x.to_string()),
                    ("y", p.y.to_string()),
                    ("file", format!("&{}", p.pixels.as_ptr() as usize)),
                    ("offset", "0".into()),
                    ("fmt", "bgra".into()),
                    ("w", p.width.to_string()),
                    ("h", p.height.to_string()),
                    ("stride", (p.width * 4).to_string()),
                ],
            )?;
        }
        if count < 2 {
            m.command(&["overlay-remove", "62"])?;
        }
        self.last = key;
        self.rendered = true;
        Ok(())
    }
    pub fn input(&mut self, m: &Mpv, kind: &str, body: &str) -> Result<Input, String> {
        let f = Frame::parse(body).ok_or("Entrada de controles no válida")?;
        let l = Layout::new(f.w, f.h).ok_or("Tamaño de controles no válido")?;
        match kind {
            "cancel" => {
                self.drag = None;
                self.fraction = None;
            }
            "down" => {
                let hit = l.hit(self.mode, f.x, f.y);
                self.drag = hit;
                self.drag_second_eye = match self.mode {
                    Projection::Ou => f.y >= (l.height / 2) as f64,
                    Projection::FramePack => f.y >= l.framepack_panels()[1].y as f64,
                    _ => f.x >= (l.width / 2) as f64,
                };
                if let Some(b @ (Button::Seek | Button::Volume)) = hit {
                    self.fraction = l.fraction(b, self.mode, f.x, f.y);
                }
            }
            "move" => {
                if let Some(b @ (Button::Seek | Button::Volume)) = self.drag {
                    self.fraction = l.drag_fraction(b, self.mode, self.drag_second_eye, f.x);
                }
            }
            "up" => {
                let pressed = self.drag.take();
                let hit = l.hit(self.mode, f.x, f.y);
                // Release can arrive before the last throttled mouse_move.
                let fraction = pressed
                    .filter(|b| matches!(b, Button::Seek | Button::Volume))
                    .and_then(|b| l.drag_fraction(b, self.mode, self.drag_second_eye, f.x))
                    .or(self.fraction.take());
                match pressed {
                    Some(Button::Seek) if f.duration > 0.0 => {
                        if let Some(p) = fraction {
                            m.seek(p * f.duration, true)?;
                        }
                    }
                    Some(Button::Volume) => {
                        if let Some(p) = fraction {
                            m.command_async(&["set", "volume", &(p * 100.0).to_string()], 2)?;
                        }
                    }
                    Some(b) if Some(b) == hit => match b {
                        Button::Play => m.command_async(&["cycle", "pause"], 2)?,
                        Button::PreviousChapter => return Ok(Input::ChapterStep(-1)),
                        Button::Chapters => return Ok(Input::Chapters),
                        Button::NextChapter => return Ok(Input::ChapterStep(1)),
                        Button::Back => m.seek(-10.0, false)?,
                        Button::Forward => m.seek(10.0, false)?,
                        Button::Mute => m.command_async(&["cycle", "mute"], 2)?,
                        Button::Fullscreen => m.command_async(&["cycle", "fullscreen"], 2)?,
                        Button::Stereo => return Ok(Input::Menu),
                        Button::Info => return Ok(Input::Info),
                        Button::Swap => return Ok(Input::Swap),
                        Button::Options => return Ok(Input::Options),
                        Button::Audio => return Ok(Input::Audio),
                        Button::Subtitles => return Ok(Input::Subtitles),
                        _ => {}
                    },
                    _ => {}
                }
            }
            "wheelup" => m.command_async(&["add", "volume", "5"], 2)?,
            "wheeldown" => m.command_async(&["add", "volume", "-5"], 2)?,
            _ => return Err("Acción de controles desconocida".into()),
        }
        self.last.clear();
        self.draw(m, body)?;
        Ok(Input::None)
    }
}
fn time(t: f64) -> String {
    let n = t.max(0.0) as u64;
    if n >= 3600 {
        format!("{}:{:02}:{:02}", n / 3600, n / 60 % 60, n % 60)
    } else {
        format!("{}:{:02}", n / 60, n % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn information_and_swap_buttons_work_from_each_output_eye() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("idle", "yes"),
        ] {
            m.option(k, v).unwrap();
        }
        m.init().unwrap();
        let l = Layout::new(1920, 1080).unwrap();
        let pixels = vec![0; (l.width * l.panel * 4) as usize];
        for mode in [
            Projection::Mono,
            Projection::Sbs,
            Projection::Ou,
            Projection::FramePack,
        ] {
            let mut c = Controls {
                mode,
                ..Default::default()
            };
            let planes = control_layout::project(&pixels, &l, mode, Glasses::RedCyan);
            for plane in planes {
                for button in [Button::Info, Button::Swap] {
                    let r = l.regions.iter().find(|(b, _)| *b == button).unwrap().1;
                    let x = plane.x as f64
                        + (r.x as f64 + r.w as f64 / 2.0) * plane.width as f64 / l.width as f64;
                    let y = plane.y as f64
                        + (r.y as f64 + r.h as f64 / 2.0) * plane.height as f64 / l.panel as f64;
                    let body = format!("1920|1080|{x}|{y}|0|30|50|1|0");
                    c.input(&m, "down", &body).unwrap();
                    let result = c.input(&m, "up", &body).unwrap();
                    assert!(
                        matches!(
                            (button, result),
                            (Button::Info, Input::Info) | (Button::Swap, Input::Swap)
                        ),
                        "Wrong button in {mode:?}"
                    );
                }
            }
        }
    }
    use crate::mpv::ClientMessage;
    use std::{
        ffi::CStr,
        path::Path,
        time::{Duration, Instant},
    };
    #[test]
    fn ten_second_buttons_seek_from_each_eye_in_sbs_ou_and_framepack() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("pause", "yes"),
            ("idle", "yes"),
        ] {
            m.option(k, v).unwrap();
        }
        m.init().unwrap();
        m.command(&[
            "loadfile",
            root.join("verificacion/muestras/pistas-qa.mkv")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        fn position(m: &mut Mpv, target: f64) {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                let event = m.wait(0.02);
                let (id, cookie) = (event.id, event.userdata);
                if id == 5 && cookie & (1_u64 << 63) != 0 {
                    m.seek_reply(cookie).unwrap();
                }
                if m.get("time-pos")
                    .and_then(|s| s.parse::<f64>().ok())
                    .is_some_and(|p| (p - target).abs() < 0.1)
                {
                    return;
                }
                assert!(Instant::now() < until, "Did not reach {target}");
            }
        }
        position(&mut m, 0.0);
        let l = Layout::new(1920, 1080).unwrap();
        let pixels = vec![255; (l.width * l.panel * 4) as usize];
        for mode in [Projection::Sbs, Projection::Ou, Projection::FramePack] {
            let mut c = Controls {
                mode,
                ..Default::default()
            };
            for plane in control_layout::project(&pixels, &l, mode, Glasses::RedCyan) {
                m.command(&["seek", "15", "absolute+exact"]).unwrap();
                position(&mut m, 15.0);
                for (button, target) in [(Button::Back, 5.0), (Button::Forward, 15.0)] {
                    let r = l.regions.iter().find(|(b, _)| *b == button).unwrap().1;
                    let x = plane.x as f64
                        + (r.x as f64 + r.w as f64 / 2.0) * plane.width as f64 / l.width as f64;
                    let y = plane.y as f64
                        + (r.y as f64 + r.h as f64 / 2.0) * plane.height as f64 / l.panel as f64;
                    let body = format!("1920|1080|{x}|{y}|15|30|50|1|0");
                    c.input(&m, "down", &body).unwrap();
                    c.input(&m, "up", &body).unwrap();
                    position(&mut m, target);
                }
            }
        }
    }
    #[test]
    fn slider_release_uses_final_coordinate_outside_panel_in_each_sbs_eye() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("idle", "yes"),
        ] {
            m.option(k, v).unwrap();
        }
        m.init().unwrap();
        let mut c = Controls {
            mode: Projection::Sbs,
            ..Default::default()
        };
        let l = Layout::new(1920, 1080).unwrap();
        let r = l
            .regions
            .iter()
            .find(|(b, _)| *b == Button::Volume)
            .unwrap()
            .1;
        for second in [false, true] {
            m.set("volume", "25").unwrap();
            let offset = if second { 960.0 } else { 0.0 };
            let x = |f: f64| {
                offset
                    + (r.x as f64 + f * r.w as f64) / 2.0
                    + control_layout::depth_shift(1920, 960, second) as f64
            };
            let body = |x: f64, y: f64| format!("1920|1080|{x}|{y}|0|120|50|1|0");
            let y = (1080 - l.panel + r.y + r.h / 2) as f64;
            c.input(&m, "down", &body(x(0.1), y)).unwrap();
            // Deliberately no mouse_move: fast release outside the panel.
            c.input(&m, "up", &body(x(0.83), 400.0)).unwrap();
            let until = Instant::now() + Duration::from_secs(2);
            while m
                .get("volume")
                .and_then(|v| v.parse::<f64>().ok())
                .is_none_or(|v| (v - 83.0).abs() > 0.1)
            {
                std::thread::sleep(Duration::from_millis(10));
                assert!(Instant::now() < until);
            }
        }
    }
    #[test]
    fn seek_release_is_precise_in_each_sbs_eye_without_last_move_event() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("idle", "yes"),
            ("pause", "yes"),
        ] {
            m.option(k, v).unwrap();
        }
        m.init().unwrap();
        m.command(&[
            "loadfile",
            root.join("verificacion/muestras/pistas-qa.mkv")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while m.get("seekable").as_deref() != Some("yes") {
            m.wait(0.05);
            assert!(Instant::now() < until);
        }
        let mut c = Controls {
            mode: Projection::Sbs,
            ..Default::default()
        };
        let l = Layout::new(1920, 1080).unwrap();
        let r = l
            .regions
            .iter()
            .find(|(b, _)| *b == Button::Seek)
            .unwrap()
            .1;
        for (second, target) in [(false, 0.73), (true, 0.31)] {
            let offset = if second { 960.0 } else { 0.0 };
            let x = |f: f64| {
                offset
                    + (r.x as f64 + f * r.w as f64) / 2.0
                    + control_layout::depth_shift(1920, 960, second) as f64
            };
            let body = |x: f64, y: f64| format!("1920|1080|{x}|{y}|0|30|50|1|0");
            c.input(
                &m,
                "down",
                &body(x(0.1), (1080 - l.panel + r.y + r.h / 2) as f64),
            )
            .unwrap();
            c.input(&m, "up", &body(x(target), 300.0)).unwrap();
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                let event = m.wait(0.05);
                let (id, cookie) = (event.id, event.userdata);
                if id == 5 && cookie & (1 << 63) != 0 {
                    m.seek_reply(cookie).unwrap();
                }
                if m.get("time-pos")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| (v - target * 30.0).abs() < 0.05)
                {
                    break;
                }
                assert!(Instant::now() < until, "No alcanza el punto SBS {target}");
            }
        }
    }
    #[test]
    fn lua_hidden_first_click_popup_lifecycle_and_idle() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        for (key, value) in [
            ("config", "no"),
            ("vo", "null"),
            ("ao", "null"),
            ("idle", "yes"),
            ("load-scripts", "no"),
            ("terminal", "no"),
        ] {
            m.option(key, value).unwrap();
        }
        m.option(
            "script-opts",
            &format!(
                "enigmax-test-client={},enigmax-test-source={}",
                m.name(),
                root.join("assets/enigmax_controls.lua")
                    .to_string_lossy()
                    .replace('\\', "/")
            ),
        )
        .unwrap();
        m.option(
            "scripts",
            root.join("verificacion/tests/menu_controls.lua")
                .to_str()
                .unwrap(),
        )
        .unwrap();
        m.init().unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let event = m.wait(0.05);
            if event.id == 16 && !event.data.is_null() {
                let message = unsafe { &*event.data.cast::<ClientMessage>() };
                if message.count == 2 && !message.args.is_null() {
                    let args = unsafe { std::slice::from_raw_parts(message.args, 2) };
                    if !args[0].is_null() && !args[1].is_null() {
                        let key = unsafe { CStr::from_ptr(args[0]) }.to_string_lossy();
                        let result = unsafe { CStr::from_ptr(args[1]) }.to_string_lossy();
                        if key == "menu-controls-test" {
                            assert_eq!(result, "ok");
                            break;
                        }
                    }
                }
            }
            assert!(
                Instant::now() < until,
                "No se recibió el resultado de las pruebas Lua"
            );
        }
    }
}
