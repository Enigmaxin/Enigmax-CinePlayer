//! Human-readable track metadata; missing information stays explicitly unknown.
use serde_json::Value;
fn clean(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(160)
        .collect::<String>()
        .trim()
        .to_owned()
}
fn field(row: &Value, key: &str) -> String {
    row[key].as_str().map(clean).unwrap_or_default()
}
pub fn language(code: &str) -> String {
    let lower = code.trim().to_lowercase().replace('_', "-");
    let name = match lower.as_str() {
        "es-es" => "Español (España)",
        "es-mx" => "Español (México)",
        "es-419" => "Español (Latinoamérica)",
        "spa" | "esp" | "es" => "Español",
        "eng" | "en" => "Inglés",
        "fra" | "fre" | "fr" => "Francés",
        "deu" | "ger" | "de" => "Alemán",
        "ita" | "it" => "Italiano",
        "por" | "pt" => "Portugués",
        "jpn" | "ja" => "Japonés",
        "zho" | "chi" | "zh" => "Chino",
        "kor" | "ko" => "Coreano",
        "rus" | "ru" => "Ruso",
        "ara" | "ar" => "Árabe",
        "cat" | "ca" => "Catalán",
        "eus" | "baq" | "eu" => "Euskera",
        "glg" | "gl" => "Gallego",
        "nld" | "dut" | "nl" => "Neerlandés",
        "pol" | "pl" => "Polaco",
        "swe" | "sv" => "Sueco",
        "dan" | "da" => "Danés",
        "fin" | "fi" => "Finés",
        "nor" | "no" => "Noruego",
        "ces" | "cze" | "cs" => "Checo",
        "hun" | "hu" => "Húngaro",
        "tur" | "tr" => "Turco",
        "und" | "" => "Idioma sin especificar",
        _ => return clean(code),
    };
    name.into()
}
pub fn label(row: &Value, track: &str) -> String {
    let mut lang = field(row, "lang");
    if lang.is_empty() {
        lang = field(row, "language");
    }
    for parent in ["metadata", "tags"] {
        if lang.is_empty() {
            lang = field(&row[parent], "language");
        }
    }
    let title = field(row, "title");
    let language = language(&lang);
    let heading = if title.is_empty() {
        format!("{language} · Pista {track}")
    } else {
        format!("{language} · {title}")
    };
    let raw = field(row, "codec");
    let profile = field(row, "codec-profile");
    let codec = match raw.to_lowercase().as_str() {
        "subrip" | "srt" => "SRT".into(),
        "ass" => "ASS".into(),
        "ssa" => "SSA".into(),
        "hdmv_pgs_subtitle" | "pgs" => "PGS".into(),
        "dvd_subtitle" | "dvdsub" => "VobSub".into(),
        "webvtt" => "WebVTT".into(),
        "mov_text" => "TX3G".into(),
        "text" => "Texto".into(),
        "ac3" => "Dolby Digital (AC-3)".into(),
        "eac3" => "Dolby Digital Plus (E-AC-3)".into(),
        "truehd" => "Dolby TrueHD".into(),
        "dts" if !profile.is_empty() => format!("DTS · {profile}"),
        "" => {
            let description = field(row, "codec-desc");
            if description.is_empty() {
                "Códec sin especificar".into()
            } else {
                description
            }
        }
        _ => raw.to_uppercase(),
    };
    let channels = if row["type"] == "audio" {
        row["demux-channel-count"]
            .as_u64()
            .or_else(|| row["audio-channels"].as_u64())
            .filter(|n| *n > 0 && *n <= 64)
            .map(|n| format!(" · {n} canales"))
            .or_else(|| {
                row["audio-channels"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(|s| format!(" · {}", clean(s)))
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    format!("{heading}\n{codec}{channels}")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_codec_and_channels_come_from_each_track() {
        let a = serde_json::json!({"type":"audio","lang":"spa","title":"Doblaje","codec":"dts","codec-profile":"DTS-HD MA","demux-channel-count":6});
        assert_eq!(
            label(&a, "2"),
            "Español · Doblaje\nDTS · DTS-HD MA · 6 canales"
        );
        let sub = serde_json::json!({"type":"sub","lang":"eng","codec":"hdmv_pgs_subtitle"});
        assert_eq!(label(&sub, "1"), "Inglés · Pista 1\nPGS");
    }
    #[test]
    fn missing_metadata_is_not_guessed_from_the_title() {
        let a = serde_json::json!({"type":"audio","title":"Atmos Spanish","codec":"truehd"});
        assert_eq!(
            label(&a, "1"),
            "Idioma sin especificar · Atmos Spanish\nDolby TrueHD"
        );
        assert_eq!(
            label(&serde_json::json!({}), "1"),
            "Idioma sin especificar · Pista 1\nCódec sin especificar"
        );
        assert_eq!(language("es_ES"), "Español (España)");
        assert_eq!(language("custom"), "custom");
    }
}
