use std::{borrow::Cow, collections::HashMap, io::Write, path::Path, sync::OnceLock};

pub const LANGUAGES: [(&str, &str); 6] = [
    ("en", "English"),
    ("es", "Español"),
    ("pt", "Português"),
    ("fr", "Français"),
    ("de", "Deutsch"),
    ("it", "Italiano"),
];
pub const FIRST_ID: u32 = 7200;
pub const LAST_ID: u32 = FIRST_ID + LANGUAGES.len() as u32 - 1;

#[cfg(not(test))]
static LANGUAGE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(test)]
thread_local! { static LANGUAGE: std::cell::Cell<usize> = const { std::cell::Cell::new(1) }; }

pub fn current() -> usize {
    #[cfg(test)]
    return LANGUAGE.with(std::cell::Cell::get);
    #[cfg(not(test))]
    LANGUAGE.load(std::sync::atomic::Ordering::Relaxed)
}

fn set(language: usize) {
    #[cfg(test)]
    LANGUAGE.with(|value| value.set(language));
    #[cfg(not(test))]
    LANGUAGE.store(language, std::sync::atomic::Ordering::Relaxed);
}

pub fn load(dir: &Path) {
    let code = std::fs::read_to_string(dir.join("config/language")).unwrap_or_default();
    set(LANGUAGES
        .iter()
        .position(|entry| entry.0 == code.trim())
        .unwrap_or(0));
}

pub fn save(dir: &Path, index: usize) -> Result<(), String> {
    let config = dir.join("config");
    std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(&config).map_err(|e| e.to_string())?;
    file.write_all(LANGUAGES[index].0.as_bytes())
        .map_err(|e| e.to_string())?;
    crate::windows::replace_file(file.path(), &config.join("language"))?;
    set(index);
    Ok(())
}

type Catalog = HashMap<&'static str, [&'static str; 5]>;
fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        include_str!("locales.tsv")
            .lines()
            .map(|line| {
                let mut fields = line.split('\t');
                let key = fields.next().expect("translation key");
                let values = std::array::from_fn(|_| fields.next().expect("missing translation"));
                assert!(fields.next().is_none(), "extra translation for {key}");
                (key, values)
            })
            .collect()
    })
}

pub fn tr(text: &str) -> Cow<'_, str> {
    let column = match current() {
        1 => return Cow::Borrowed(text),
        0 => 0,
        index => index - 1,
    };
    if let Some(row) = catalog().get(text) {
        return Cow::Borrowed(row[column]);
    }
    for separator in ["\n", "\t", "\0", " · "] {
        if text.contains(separator) {
            let parts: Vec<_> = text.split(separator).map(tr).collect();
            if parts.iter().zip(text.split(separator)).any(|(a, b)| a != b) {
                return Cow::Owned(parts.join(separator));
            }
            return Cow::Borrowed(text);
        }
    }
    if let Some((label, value)) = text.split_once(": ") {
        if let Some(row) = catalog().get(label) {
            let punctuation = if current() == 3 { " :" } else { ":" };
            return Cow::Owned(format!("{}{punctuation} {}", row[column], tr(value)));
        }
    }
    Cow::Borrowed(text)
}

pub fn known_message(text: &str) -> Option<Cow<'_, str>> {
    if catalog().contains_key(text) {
        return Some(tr(text));
    }
    let (label, path) = text.split_once(": ")?;
    if label == "No existe el archivo" {
        let name = Path::new(path).file_name().and_then(|name| name.to_str());
        return Some(name.map_or_else(
            || tr(label),
            |name| Cow::Owned(format!("{}\n{name}", tr(label))),
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogs_have_unique_nonempty_entries_in_every_language() {
        assert_eq!(catalog().len(), include_str!("locales.tsv").lines().count());
        assert!(catalog().values().flatten().all(|v| !v.trim().is_empty()));
        for index in 0..LANGUAGES.len() {
            set(index);
            assert_ne!(tr("Configuración"), "");
            assert!(tr("Abrir vídeo…\tCtrl+O").ends_with("\tCtrl+O"));
            assert!(tr("Posición: 95 %").ends_with(": 95 %"));
        }
    }

    #[test]
    fn recent_labels_and_language_preference_are_verified_in_every_locale() {
        let dir = tempfile::tempdir().unwrap();
        load(dir.path());
        assert_eq!(current(), 0);
        let expected = [
            [
                "Opening video",
                "Crosstalk",
                "Per-eye brightness",
                "Original",
            ],
            [
                "Abriendo vídeo",
                "Crosstalk",
                "Ajuste de brillo por ojo",
                "Original",
            ],
            ["Abrindo vídeo", "Diafonia", "Brilho por olho", "Original"],
            [
                "Ouverture de la vidéo",
                "Diaphonie",
                "Luminosité par œil",
                "D’origine",
            ],
            [
                "Video wird geöffnet",
                "Übersprechen",
                "Helligkeit je Auge",
                "Original",
            ],
            [
                "Apertura del video",
                "Diafonia",
                "Luminosità per occhio",
                "Originale",
            ],
        ];
        for (index, labels) in expected.iter().enumerate() {
            save(dir.path(), index).unwrap();
            set((index + 1) % LANGUAGES.len());
            load(dir.path());
            assert_eq!(current(), index);
            for (source, target) in [
                "Abriendo vídeo",
                "Crosstalk",
                "Ajuste de brillo por ojo",
                "Original",
            ]
            .iter()
            .zip(labels)
            {
                assert_eq!(tr(source), *target);
            }
        }
    }
}
