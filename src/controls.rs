//! Bounded screen-space UI, drawn only while visible. No video readback.
use crate::{mpv::Mpv, player::State};
use cineplayer_core::{
    control_layout::{self, Button, Layout, Projection, Rect},
    stereo::{Glasses, Output},
};
use std::{ffi::c_void, ptr};
type Handle = *mut c_void;
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
struct Canvas {
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
                -(h * 22 / 100).clamp(14, 36).min((w / 18).max(12)),
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
    fn pixels(&self) -> &[u8] {
        unsafe {
            GdiFlush();
            std::slice::from_raw_parts(self.bits, self.width as usize * self.height as usize * 4)
        }
    }
    fn clear(&mut self) {
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(
                self.bits,
                self.width as usize * self.height as usize * 4,
            )
        };
        for p in pixels.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[18, 18, 18, 255]);
        }
    }
    fn rect(&mut self, r: Rect, value: u8) {
        let w = self.width;
        let h = self.height;
        let p = unsafe { std::slice::from_raw_parts_mut(self.bits, w as usize * h as usize * 4) };
        for y in r.y.max(0)..(r.y + r.h).min(h) {
            for x in r.x.max(0)..(r.x + r.w).min(w) {
                let i = (y * w + x) as usize * 4;
                p[i..i + 4].copy_from_slice(&[value, value, value, 255]);
            }
        }
    }
    fn text(&self, x: i32, y: i32, text: &str) {
        let s = text.encode_utf16().collect::<Vec<_>>();
        unsafe { TextOutW(self.dc, x, y, s.as_ptr(), s.len() as i32) };
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
        if a.len() != 9 {
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
        })
    }
}
pub enum Input {
    None,
    Menu,
    Audio,
    Subtitles,
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
    fraction: Option<f64>,
    rendered: bool,
}
impl Controls {
    pub fn rendered(&self) -> bool {
        self.rendered
    }
    pub fn ready(&mut self) {
        self.ready = true;
        self.config.clear();
    }
    pub fn sync(&mut self, m: &Mpv, s: &State) -> Result<(), String> {
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
                    if layout.horizontal() {
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
        let key = format!(
            "{body}/{:?}/{:?}/{:?}",
            self.mode, self.glasses, self.fraction
        );
        if key == self.last {
            return Ok(());
        }
        let l = Layout::new(f.w, f.h).ok_or("Tamaño de controles no válido")?;
        if self
            .canvas
            .as_ref()
            .is_none_or(|c| c.width != l.width || c.height != l.panel)
        {
            self.canvas = Some(Canvas::new(l.width, l.panel)?);
        }
        let c = self.canvas.as_mut().ok_or("No hay lienzo de controles")?;
        c.clear();
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
        let mut texts = Vec::new();
        for (button, r) in &l.regions {
            let bright = if hover == Some(*button) { 255 } else { 195 };
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
                    c.rect(
                        Rect {
                            x: r.x,
                            y: cy - 2,
                            w: filled,
                            h: 4,
                        },
                        225,
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
                Button::Back => texts.push((r.x, cy - l.panel / 10, "−10".into())),
                Button::Forward => texts.push((r.x, cy - l.panel / 10, "+10".into())),
                Button::Stereo => texts.push((r.x + r.w / 5, cy - l.panel / 10, "3D".into())),
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
        let shown = self
            .fraction
            .filter(|_| self.drag == Some(Button::Seek))
            .map(|p| p * f.duration)
            .unwrap_or(f.time);
        if l.time_width >= l.size * 2 {
            texts.push((
                l.time_x,
                l.panel * 58 / 100 + l.size / 2 - l.panel / 10,
                if l.time_width < l.size * 5 {
                    time(shown)
                } else {
                    format!("{} / {}", time(shown), time(f.duration))
                },
            ));
        }
        if let Some(button @ (Button::Audio | Button::Subtitles)) = hover {
            // A descriptive hint also explains compact A/S buttons in small windows.
            texts.push((
                l.width / 2 - l.size * 3,
                l.panel / 3,
                if button == Button::Audio {
                    "Pista de audio"
                } else {
                    "Subtítulos"
                }
                .into(),
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
                if let Some(b @ (Button::Seek | Button::Volume)) = hit {
                    self.fraction = l.fraction(b, self.mode, f.x, f.y);
                }
            }
            "move" => {
                if let Some(b @ (Button::Seek | Button::Volume)) = self.drag {
                    self.fraction = l.fraction(b, self.mode, f.x, f.y);
                }
            }
            "up" => {
                let pressed = self.drag.take();
                let hit = l.hit(self.mode, f.x, f.y);
                let fraction = self.fraction.take();
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
                        Button::Back => m.seek(-10.0, false)?,
                        Button::Forward => m.seek(10.0, false)?,
                        Button::Mute => m.command_async(&["cycle", "mute"], 2)?,
                        Button::Fullscreen => m.command_async(&["cycle", "fullscreen"], 2)?,
                        Button::Stereo => return Ok(Input::Menu),
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
    use crate::mpv::ClientMessage;
    use std::{
        ffi::CStr,
        path::Path,
        time::{Duration, Instant},
    };
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
