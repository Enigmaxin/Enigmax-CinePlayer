//! Event-driven menus composited after video conversion, in each output eye.
use crate::{controls::Canvas, mpv::Mpv};
use cineplayer_core::{
    control_layout::{Layout, Projection, Rect},
    popup_geometry::Viewport,
};

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: u32,
    pub label: String,
    pub checked: bool,
    pub enabled: bool,
    pub children: Vec<Entry>,
}
impl Entry {
    pub fn action(id: u32, label: String) -> Self {
        Self {
            id,
            label,
            checked: false,
            enabled: true,
            children: vec![],
        }
    }
}
pub fn projection(m: &Mpv) -> Projection {
    match m.user_data("user-data/enigmax-projection").as_deref() {
        Some("sbs") => Projection::Sbs,
        Some("ou") => Projection::Ou,
        Some("framepack") => Projection::FramePack,
        Some("anaglyph") => Projection::Anaglyph,
        _ => Projection::Mono,
    }
}
pub fn stereo(m: &Mpv) -> bool {
    matches!(
        projection(m),
        Projection::Sbs | Projection::Ou | Projection::FramePack | Projection::Anaglyph
    )
}
struct Guard<'a>(&'a Mpv);
impl Drop for Guard<'_> {
    fn drop(&mut self) {
        let _ = self.0.set("user-data/enigmax-popup-state", "");
        for id in [59, 60] {
            let _ = self.0.command(&["overlay-remove", &id.to_string()]);
        }
        let _ = self
            .0
            .command(&["script-message-to", "enigmax_controls", "stereo-popup", ""]);
    }
}
struct Page {
    title: String,
    items: Vec<Entry>,
    selected: Option<usize>,
    offset: usize,
}
impl Page {
    fn new(title: String, items: Vec<Entry>) -> Self {
        let selected = items.iter().position(|e| e.enabled);
        Self {
            title,
            items,
            selected,
            offset: 0,
        }
    }
}
fn dimensions(m: &Mpv) -> (i32, i32) {
    let get = |k| m.get(k).and_then(|v| v.parse().ok()).unwrap_or(0);
    (get("osd-width"), get("osd-height"))
}
struct Geometry {
    rect: Rect,
    header: i32,
    row: i32,
    visible: usize,
    font: i32,
}
fn geometry(w: i32, h: i32, count: usize, wide: bool) -> Result<Geometry, String> {
    let l = Layout::new(w, h).ok_or("Tamaño del menú no válido")?;
    let margin = (w / 80).clamp(8, 32);
    let width = if wide {
        (w * 2 / 3).clamp(400, 1000)
    } else {
        (w / 3).clamp(240, 720)
    }
    .min(w - 2 * margin);
    let header = (h / 14).clamp(24, 56);
    let row = if wide {
        (h / 27).clamp(22, 38)
    } else {
        (h / 13).clamp(22, 64)
    };
    let visible = ((h - l.panel - header - 3 * margin) / row).max(1) as usize;
    let visible = visible.min(if wide { 28 } else { 12 }).min(count.max(1));
    let height = header + row * visible as i32 + 8;
    Ok(Geometry {
        rect: Rect {
            x: w - margin - width,
            y: (h - l.panel - height - margin).max(0),
            w: width,
            h: height,
        },
        header,
        row,
        visible,
        font: (if wide { row * 3 / 5 } else { row / 3 }).clamp(11, 24),
    })
}
fn activate(pages: &mut Vec<Page>, index: usize) -> Option<u32> {
    let page = pages.last()?;
    let entry = page.items.get(index)?;
    if !entry.enabled {
        return None;
    }
    if !entry.children.is_empty() {
        pages.push(Page::new(entry.label.clone(), entry.children.clone()));
        None
    } else {
        Some(entry.id)
    }
}
pub fn show(m: &Mpv, title: &str, entries: Vec<Entry>) -> Result<u32, String> {
    show_sized(m, title, entries, false, None)
}
pub fn show_information(m: &Mpv, title: &str, entries: Vec<Entry>) -> Result<u32, String> {
    show_sized(m, title, entries, true, None)
}
pub fn show_anchored(
    m: &Mpv,
    title: &str,
    entries: Vec<Entry>,
    button: cineplayer_core::control_layout::Button,
) -> Result<u32, String> {
    show_sized(m, title, entries, false, Some(button))
}
fn show_sized(
    m: &Mpv,
    title: &str,
    entries: Vec<Entry>,
    wide: bool,
    anchor: Option<cineplayer_core::control_layout::Button>,
) -> Result<u32, String> {
    if entries.is_empty() {
        return Ok(0);
    }
    let mut input = m.popup_client()?;
    let name = input.name();
    let _guard = Guard(m);
    m.command(&[
        "script-message-to",
        "enigmax_controls",
        "stereo-popup",
        &name,
    ])?;
    let mut pages = vec![Page::new(title.to_owned(), entries)];
    let (mut w, mut h) = dimensions(m);
    let mode = projection(m);
    let opener = anchor.or_else(|| {
        Layout::new(w, h).and_then(|l| {
            l.hit(
                mode,
                m.get("mouse-pos/x")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(-1.0),
                m.get("mouse-pos/y")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(-1.0),
            )
        })
    });
    let mut canvas: Option<Canvas> = None;
    let mut canvas_size = (0, 0);
    let mut dirty = true;
    let mut mouse_down = false;
    #[cfg(test)]
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
    loop {
        #[cfg(test)]
        if std::time::Instant::now() > deadline {
            return Err("Timeout del test de menú".into());
        }
        let view = Viewport::new(w, h, mode).ok_or("La superficie del menú no está disponible")?;
        let depth = pages.len();
        let page = pages.last_mut().ok_or("Menú vacío")?;
        let mut g = geometry(w, h, page.items.len(), wide)?;
        if !wide
            && let Some(button) = opener
            && let Some(layout) = Layout::new(w, h)
            && let Some((_, anchor)) = layout.regions.iter().find(|(b, _)| *b == button)
        {
            g.rect.x = anchor.x.clamp(8, (w - g.rect.w - 8).max(8));
        }
        page.offset = page.offset.min(page.items.len().saturating_sub(g.visible));
        if dirty {
            if canvas_size != (g.rect.w, g.rect.h) {
                canvas = Some(Canvas::with_font(g.rect.w, g.rect.h, g.font)?);
                canvas_size = (g.rect.w, g.rect.h);
            }
            let c = canvas.as_mut().ok_or("Falta el lienzo del menú")?;
            c.rounded(
                Rect {
                    x: 0,
                    y: 0,
                    w: g.rect.w,
                    h: g.rect.h,
                },
                8,
                [34, 31, 27, 255],
            );
            c.text_clipped(
                14,
                8,
                g.rect.w - 60,
                g.header - 8,
                &format!(
                    "{}{}",
                    if depth > 1 { "‹  " } else { "" },
                    page.title.split('\t').next().unwrap_or_default()
                ),
            );
            c.rounded(
                Rect {
                    x: g.rect.w - 36,
                    y: 5,
                    w: 30,
                    h: (g.header - 10).max(18),
                },
                5,
                [59, 45, 107, 255],
            );
            c.text(g.rect.w - 26, 8, "×");
            c.rect(
                Rect {
                    x: 8,
                    y: g.header - 3,
                    w: g.rect.w - 16,
                    h: 1,
                },
                65,
            );
            for (index, item) in page
                .items
                .iter()
                .enumerate()
                .skip(page.offset)
                .take(g.visible)
            {
                let y = g.header + (index - page.offset) as i32 * g.row;
                if item.id == 1231 {
                    c.rounded(
                        Rect {
                            x: 5,
                            y,
                            w: g.rect.w - 10,
                            h: g.row - 2,
                        },
                        5,
                        [67, 78, 43, 255],
                    );
                }
                if page.selected == Some(index) && item.enabled {
                    c.rounded(
                        Rect {
                            x: 5,
                            y,
                            w: g.rect.w - 10,
                            h: g.row - 2,
                        },
                        5,
                        [56, 52, 47, 255],
                    );
                }
                let mut lines = item.label.split('\n');
                let label = lines
                    .next()
                    .unwrap_or_default()
                    .split('\t')
                    .next()
                    .unwrap_or_default();
                let second = lines.next();
                c.text_clipped(
                    28,
                    y + if second.is_some() {
                        5
                    } else {
                        (g.row - g.font) / 2
                    },
                    g.rect.w - 60,
                    if second.is_some() {
                        g.row / 2
                    } else {
                        g.row - (g.row - g.font) / 2
                    },
                    &label.chars().take(160).collect::<String>(),
                );
                if let Some(second) = second {
                    c.text_clipped(
                        28,
                        y + g.row / 2,
                        g.rect.w - 60,
                        g.row / 2,
                        &second.chars().take(96).collect::<String>(),
                    );
                }
                if item.checked {
                    c.text(8, y + 5, "✓");
                }
                if !item.children.is_empty() {
                    c.text(g.rect.w - 23, y + 5, "›");
                }
            }
            if page.items.len() > g.visible {
                c.text(g.rect.w / 2 - 10, g.rect.h - g.font, "⋮");
            }
            let source = c.pixels();
            let ana = if mode == Projection::Anaglyph {
                let glasses = m
                    .user_data("user-data/enigmax-glasses")
                    .and_then(|s| s.parse::<usize>().ok())
                    .and_then(|n| cineplayer_core::stereo::Glasses::ALL.get(n).copied())
                    .unwrap_or_default();
                Some(cineplayer_core::control_layout::anaglyph_hud(
                    source, g.rect.w, g.rect.h, glasses,
                ))
            } else {
                None
            };
            for (i, plane) in view
                .project(g.rect, ana.as_deref().unwrap_or(source))
                .iter()
                .enumerate()
            {
                m.command_named(
                    "overlay-add",
                    &[
                        ("id", (59 + i).to_string()),
                        ("x", plane.x.to_string()),
                        ("y", plane.y.to_string()),
                        ("file", format!("&{}", plane.pixels.as_ptr() as usize)),
                        ("offset", "0".into()),
                        ("fmt", "bgra".into()),
                        ("w", plane.width.to_string()),
                        ("h", plane.height.to_string()),
                        ("stride", (plane.width * 4).to_string()),
                    ],
                )?;
            }
            dirty = false;
            #[cfg(test)]
            m.set("user-data/enigmax-popup-state",&serde_json::json!({"client":name,"w":w,"h":h,"mode":mode.code(),"x":g.rect.x,"y":g.rect.y,"width":g.rect.w,"height":g.rect.h,"header":g.header,"row":g.row,"visible":g.visible,"offset":page.offset,"depth":depth,"selected":page.selected,"ids":page.items.iter().map(|e|e.id).collect::<Vec<_>>(),"labels":page.items.iter().map(|e|&e.label).collect::<Vec<_>>(),"enabled":page.items.iter().map(|e|e.enabled).collect::<Vec<_>>()}).to_string())?;
        }
        let Some(args) = (match input.next() {
            Ok(v) => v,
            Err(_) => return Ok(0),
        }) else {
            continue;
        };
        if args.first().map(String::as_str) != Some("popup-input") {
            continue;
        }
        let kind = args.get(1).map(String::as_str).unwrap_or_default();
        if kind == "close" {
            return Ok(0);
        }
        if kind == "left" {
            if pages.len() > 1 {
                pages.pop();
                dirty = true;
            } else {
                return Ok(0);
            }
            continue;
        }
        let page = pages.last_mut().ok_or("Menú vacío")?;
        if matches!(kind, "enter" | "right") {
            if let Some(index) = page.selected
                && let Some(id) = activate(&mut pages, index)
            {
                return Ok(id);
            }
            dirty = true;
            continue;
        }
        if matches!(kind, "wheelup" | "wheeldown") {
            page.offset = if kind == "wheelup" {
                page.offset.saturating_sub(1)
            } else {
                (page.offset + 1).min(page.items.len().saturating_sub(g.visible))
            };
            if page
                .selected
                .is_some_and(|n| n < page.offset || n >= page.offset + g.visible)
            {
                page.selected = None;
            }
            dirty = true;
            continue;
        }
        if matches!(kind, "previous" | "next") {
            let reverse = kind == "previous";
            let n = page.items.len();
            let mut index = page.selected.unwrap_or(if reverse { 0 } else { n - 1 });
            for _ in 0..n {
                index = if reverse {
                    (index + n - 1) % n
                } else {
                    (index + 1) % n
                };
                if page.items[index].enabled {
                    break;
                }
            }
            page.selected = Some(index);
            page.offset = page.offset.min(index);
            if index >= page.offset + g.visible {
                page.offset = index + 1 - g.visible;
            }
            dirty = true;
            continue;
        }
        let Some(body) = args.get(2) else {
            continue;
        };
        let values = body
            .split('|')
            .take(4)
            .map(|s| s.parse::<f64>().ok().filter(|n| n.is_finite()))
            .collect::<Option<Vec<_>>>();
        let Some(values) = values.filter(|a| a.len() == 4) else {
            continue;
        };
        let Some(new_view) = Viewport::new(values[0] as i32, values[1] as i32, mode) else {
            continue;
        };
        if (w, h) != (new_view.width, new_view.height) {
            w = new_view.width;
            h = new_view.height;
            if kind == "down" {
                mouse_down = true;
            }
            dirty = true;
            continue;
        }
        let point = view.point(values[2], values[3]);
        let index = point.and_then(|(x, y)| {
            if !g.rect.contains(x, y) || y < (g.rect.y + g.header) as f64 {
                return None;
            }
            let index =
                page.offset + ((y - g.rect.y as f64 - g.header as f64) / g.row as f64) as usize;
            (index < page.items.len() && index < page.offset + g.visible).then_some(index)
        });
        if kind == "move" && page.selected != index {
            page.selected = index;
            dirty = true;
        }
        if kind == "down" {
            mouse_down = true;
        }
        if kind == "up" && mouse_down {
            mouse_down = false;
            if let Some(index) = index {
                if let Some(id) = activate(&mut pages, index) {
                    return Ok(id);
                }
                dirty = true;
            } else if let Some((x, y)) = point {
                if g.rect.contains(x, y)
                    && y < (g.rect.y + g.header) as f64
                    && x < (g.rect.x + 40) as f64
                    && pages.len() > 1
                {
                    pages.pop();
                    dirty = true;
                } else {
                    if !g.rect.contains(x, y)
                        && let Some(hit) =
                            Layout::new(w, h).and_then(|l| l.hit(mode, values[2], values[3]))
                        && Some(hit) != opener
                        && hit != cineplayer_core::control_layout::Button::Stereo
                    {
                        m.command(&[
                            "script-message-to",
                            &m.name(),
                            "controls-input",
                            "down",
                            body,
                        ])?;
                        m.command(&["script-message-to", &m.name(), "controls-input", "up", body])?;
                    }
                    return Ok(0);
                }
            } else {
                return Ok(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        configure,
        controls::Controls,
        mpv::ClientMessage,
        player::{self, State},
        windows::Menu,
    };
    use cineplayer_core::{
        control_layout::{Button, depth_shift},
        stereo::{Layout as StereoLayout, Output},
    };
    use std::{
        ffi::CStr,
        path::Path,
        time::{Duration, Instant},
    };
    fn wait_message(m: &mut Mpv, wanted: &str) {
        let until = Instant::now() + Duration::from_secs(8);
        loop {
            let event = m.wait(0.02);
            if event.id == 16 && !event.data.is_null() {
                let msg = unsafe { &*event.data.cast::<ClientMessage>() };
                if msg.count > 0
                    && !msg.args.is_null()
                    && unsafe { CStr::from_ptr(*msg.args) }.to_bytes() == wanted.as_bytes()
                {
                    return;
                }
            }
            assert!(Instant::now() < until, "Message timeout: {wanted}");
        }
    }
    #[test]
    fn wrong_input_format_can_be_corrected_again_from_the_3d_button() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let temp = tempfile::tempdir().unwrap();
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        wait_message(&mut m, "controls-ready");
        let mut state = State::new(temp.path(), true);
        player::load(
            &m,
            &mut state,
            root.join("verificacion/muestras/ass3d-fullsbs.mkv")
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
        state.pending_load = false;
        state.prefs.layout = StereoLayout::Mono;
        m.command(&[
            "load-script",
            root.join("verificacion/tests/stereo_popup.lua")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        wait_message(&mut m, "popup-qa-ready");
        for (id, expected) in [
            (1002, StereoLayout::FullSbs),
            (1001, StereoLayout::Mono),
            (1002, StereoLayout::FullSbs),
        ] {
            m.set(
                "user-data/enigmax-popup-test",
                &serde_json::json!({"sequence":[1232,id,1210],"eye":0}).to_string(),
            )
            .unwrap();
            player::choose_view(&m, &mut state).unwrap();
            assert_eq!(state.prefs.layout, expected);
            assert_eq!(state.prefs.output, Output::Left);
            assert_eq!(
                m.user_data("user-data/enigmax-toolbar-popup").as_deref(),
                Some("no")
            );
        }
    }
    #[test]
    fn real_stereo_popups_select_from_each_eye_navigate_and_toggle_closed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut m = Mpv::new(&root.join("libmpv-2.dll")).unwrap();
        configure(&m, true, root).unwrap();
        m.init().unwrap();
        let mut controls = Controls::default();
        wait_message(&mut m, "controls-ready");
        controls.ready();
        let mut state = State::new(root, true);
        let dest = root.join("verificacion/0.2.14/popup-tests");
        std::fs::create_dir_all(&dest).unwrap();
        for (case, layout, file) in [
            ("hsbs", StereoLayout::HalfSbs, "ass3d-hsbs.mkv"),
            ("fullsbs", StereoLayout::FullSbs, "ass3d-fullsbs.mkv"),
            ("hou", StereoLayout::HalfOu, "ass3d-hou.mkv"),
            ("ou", StereoLayout::FullOu, "ass3d-fullou.mkv"),
        ] {
            state.prefs.layout = layout;
            state.prefs.output = Output::Original;
            m.command(&[
                "loadfile",
                root.join("verificacion/muestras")
                    .join(file)
                    .to_str()
                    .unwrap(),
            ])
            .unwrap();
            let until = Instant::now() + Duration::from_secs(10);
            loop {
                let id = m.wait(0.02).id;
                if id == 8 {
                    break;
                }
                assert!(Instant::now() < until);
            }
            state.source = root
                .join("verificacion/muestras")
                .join(file)
                .to_str()
                .unwrap()
                .into();
            state.applied = None;
            player::apply(&m, &mut state).unwrap();
            controls.sync(&m, &state).unwrap();
            assert!(
                stereo(&m),
                "Projection was not published: {:?}; source {:?}",
                m.get("user-data/enigmax-projection"),
                state.input_layout(&m)
            );
            let until = Instant::now() + Duration::from_secs(3);
            while dimensions(&m).0 < 160 {
                m.wait(0.02);
                assert!(Instant::now() < until);
            }
            if case == "hsbs" {
                m.command(&[
                    "load-script",
                    root.join("verificacion/tests/stereo_popup.lua")
                        .to_str()
                        .unwrap(),
                ])
                .unwrap();
                wait_message(&mut m, "popup-qa-ready");
            }
            for eye in [0, 1] {
                m.set("user-data/enigmax-popup-test",&serde_json::json!({"plan":[702],"eye":eye,"capture":dest.join(format!("{case}-{eye}.png"))}).to_string()).unwrap();
                let menu = Menu::new().unwrap();
                menu.item(701, "Primera pista\nAAC · 2 canales", true)
                    .unwrap();
                menu.item(702, "Segunda pista\nFLAC · 6 canales", false)
                    .unwrap();
                assert_eq!(menu.show(&m).unwrap(), 702, "{case}, eye {eye}");
                assert_eq!(
                    m.user_data("user-data/enigmax-popup-state").as_deref(),
                    Some("")
                );
            }
            m.set("user-data/enigmax-popup-test",&serde_json::json!({"plan":[703,704],"eye":1,"capture":dest.join(format!("{case}-submenu.png"))}).to_string()).unwrap();
            let root_menu = Menu::new().unwrap();
            let child = Menu::new().unwrap();
            child.item(704, "Formato elegido", false).unwrap();
            root_menu.submenu("Más opciones", child).unwrap();
            // Win32 assigns no command ID to submenu rows; use the sentinel returned by GetMenuItemID.
            m.set("user-data/enigmax-popup-test",&serde_json::json!({"plan":[u32::MAX,704],"eye":1,"capture":dest.join(format!("{case}-submenu.png"))}).to_string()).unwrap();
            assert_eq!(root_menu.show(&m).unwrap(), 704);
            let (w, h) = dimensions(&m);
            let l = Layout::new(w, h).unwrap();
            let r = l
                .regions
                .iter()
                .find(|(b, _)| *b == Button::Stereo)
                .unwrap()
                .1;
            let mode = projection(&m);
            let second = true;
            let x = (r.x + r.w / 2) as f64;
            let y = (h - l.panel + r.y + r.h / 2) as f64;
            let (x, y) = if mode == Projection::Sbs {
                let span = w - w / 2;
                (
                    (w / 2) as f64
                        + x * span as f64 / w as f64
                        + depth_shift(w, span, second) as f64,
                    y,
                )
            } else {
                (
                    x + depth_shift(w, w, second) as f64,
                    (h / 2) as f64 + y / 2.0,
                )
            };
            let body = format!("{w}|{h}|{x}|{y}|0|30|50|1|0|0");
            m.set("user-data/enigmax-popup-test",&serde_json::json!({"close_button":body,"capture":dest.join(format!("{case}-toggle.png"))}).to_string()).unwrap();
            assert_eq!(
                show(
                    &m,
                    "Vídeo 3D",
                    vec![
                        Entry::action(1020, "3D original".into()),
                        Entry::action(1210, "Reproducir en 2D".into())
                    ]
                )
                .unwrap(),
                0,
                "{case} toggle"
            );
            println!("{case}: both eyes, nested menu, second 3D button press OK");
        }
        let menu = Menu::new().unwrap();
        for id in 800..840 {
            menu.item(id, &format!("Capítulo {}", id - 799), false)
                .unwrap();
        }
        m.set(
            "user-data/enigmax-popup-test",
            &serde_json::json!({"plan":[839],"eye":1,"capture":dest.join("scroll.png")})
                .to_string(),
        )
        .unwrap();
        assert_eq!(menu.show(&m).unwrap(), 839);
        m.set("user-data/enigmax-popup-test",&serde_json::json!({"sequence":[1230,1231,1210],"eye":0,"capture":dest.join("view-back-2d.png")}).to_string()).unwrap();
        player::choose_view(&m, &mut state).unwrap();
        assert_eq!(state.prefs.output, Output::Left);
        assert_eq!(
            m.user_data("user-data/enigmax-toolbar-popup").as_deref(),
            Some("no")
        );
        controls.sync(&m, &state).unwrap();
        m.set("user-data/enigmax-popup-test",&serde_json::json!({"sequence":[1230,1201],"eye":0,"capture":dest.join("view-green-magenta.png")}).to_string()).unwrap();
        player::choose_view(&m, &mut state).unwrap();
        assert_eq!(state.prefs.output, Output::Anaglyph);
        assert_eq!(
            state.prefs.glasses,
            cineplayer_core::stereo::Glasses::GreenMagenta
        );
        controls.sync(&m, &state).unwrap();
        assert_eq!(
            m.user_data("user-data/enigmax-glasses").as_deref(),
            Some("1")
        );
        println!("Scroll, glasses, Back, 2D and reopening from 2D: OK");
        m.command(&[
            "loadfile",
            root.join("verificacion/muestras/pistas-qa.mkv")
                .to_str()
                .unwrap(),
        ])
        .unwrap();
        let until = Instant::now() + Duration::from_secs(8);
        loop {
            if m.wait(0.02).id == 8 {
                break;
            }
            assert!(Instant::now() < until);
        }
        state.prefs.layout = StereoLayout::HalfSbs;
        state.prefs.output = Output::Original;
        state.applied = None;
        player::apply(&m, &mut state).unwrap();
        controls.sync(&m, &state).unwrap();
        m.set("pause", "no").unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while m
            .get("audio-pts")
            .and_then(|s| s.parse::<f64>().ok())
            .is_none_or(|p| p < 0.1)
        {
            m.wait(0.02);
            assert!(Instant::now() < until);
        }
        let before = m.get("audio-pts").unwrap().parse::<f64>().unwrap();
        m.set("user-data/enigmax-popup-test",&serde_json::json!({"plan":[702],"eye":1,"delay":1.2,"capture":dest.join("live-audio.png")}).to_string()).unwrap();
        let menu = Menu::new().unwrap();
        menu.item(702, "Pista de audio", true).unwrap();
        assert_eq!(menu.show(&m).unwrap(), 702);
        let after = m.get("audio-pts").unwrap().parse::<f64>().unwrap();
        assert!(
            after - before > 0.5,
            "Audio stopped while menu was open: {before} -> {after}"
        );
        assert_eq!(m.get("pause").as_deref(), Some("no"));
        println!(
            "Audio continues during stereo popup: {:.3} s advanced",
            after - before
        );
    }
}
