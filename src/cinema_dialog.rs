//! On-demand native cards, with no browser runtime or background animation.
use std::ffi::c_void;

#[repr(C)]
struct NativeChoice {
    id: i32,
    title: *const u16,
    description: *const u16,
    image: i32,
}
unsafe extern "C" {
    fn cine_choose(
        owner: *mut c_void,
        heading: *const u16,
        subtitle: *const u16,
        choices: *const NativeChoice,
        count: i32,
        glasses: i32,
        labels: *const *const u16,
    ) -> i32;
}
pub struct Choice<'a> {
    pub id: i32,
    pub title: &'a str,
    pub description: &'a str,
    pub image: i32,
}
pub fn show(
    m: &crate::mpv::Mpv,
    heading: &str,
    subtitle: &str,
    choices: &[Choice<'_>; 5],
    glasses: bool,
    native: bool,
) -> Result<u32, String> {
    if !native
        && (crate::popup::stereo(m)
            || m.user_data("user-data/enigmax-toolbar-popup").as_deref() == Some("yes"))
    {
        let mut entries = choices
            .iter()
            .map(|c| {
                crate::popup::Entry::action(
                    c.id as u32,
                    if c.description.is_empty() {
                        c.title.into()
                    } else {
                        format!("{}\n{}", c.title, c.description)
                    },
                )
            })
            .collect::<Vec<_>>();
        if !glasses {
            entries.push(crate::popup::Entry::action(
                1232,
                "Formato 3D de entrada".into(),
            ));
            entries.push(crate::popup::Entry::action(
                1233,
                "Todas las opciones 3D".into(),
            ));
        }
        if glasses {
            entries.insert(0, crate::popup::Entry::action(1231, "← Volver".into()));
        }
        return crate::popup::show(m, heading, entries);
    }
    fn wide(text: &str) -> Vec<u16> {
        crate::i18n::tr(text)
            .encode_utf16()
            .chain(Some(0))
            .collect()
    }
    let labels = [
        "Enigmax CinePlayer — Vídeo 3D",
        "Cerrar",
        "← Volver",
        "Todas las opciones 3D",
        "Frame Packing: composición experimental; no activa HDMI 3D.",
    ]
    .map(wide);
    let label_ptrs = labels.each_ref().map(|s| s.as_ptr());
    let headings = (wide(heading), wide(subtitle));
    let texts = choices
        .iter()
        .map(|c| (wide(c.title), wide(c.description)))
        .collect::<Vec<_>>();
    let buttons = choices
        .iter()
        .zip(&texts)
        .map(|(c, t)| NativeChoice {
            id: c.id,
            title: t.0.as_ptr(),
            description: t.1.as_ptr(),
            image: c.image,
        })
        .collect::<Vec<_>>();
    let owner = m
        .get("window-id")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    m.command(&["script-message-to", "enigmax_controls", "popup", "yes"])?;
    // All pointers remain alive throughout the blocking native modal call.
    let id = unsafe {
        cine_choose(
            owner as *mut c_void,
            headings.0.as_ptr(),
            headings.1.as_ptr(),
            buttons.as_ptr(),
            5,
            i32::from(glasses),
            label_ptrs.as_ptr(),
        )
    };
    let restored = m.command(&["script-message-to", "enigmax_controls", "popup", "no"]);
    restored?;
    if id < 0 {
        Err(format!(
            "{} ({id})",
            crate::i18n::tr("No se pudo abrir el selector de cine")
        ))
    } else {
        Ok(id as u32)
    }
}
