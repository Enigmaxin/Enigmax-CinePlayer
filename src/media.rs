//! Bounded metadata for the HUD. Never infer a title from a transport URI.
pub fn title(source: &str, metadata: &str) -> String {
    let mkv = std::path::Path::new(source)
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("mkv"));
    let tags: serde_json::Value = serde_json::from_str(metadata).unwrap_or_default();
    let tagged = mkv
        .then(|| {
            tags.as_object()
                .and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case("title")))
                .and_then(|(_, v)| v.as_str())
        })
        .flatten();
    let fallback = source.rsplit(['/', '\\']).next().unwrap_or(source);
    let clean = |s: &str| {
        s.chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .take(512)
            .collect::<String>()
            .trim()
            .to_owned()
    };
    tagged
        .map(clean)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| clean(fallback))
}
pub fn size(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.2} GiB", bytes as f64 / (1_u64 << 30) as f64)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1_u64 << 20) as f64)
    }
}
pub fn bitrate(value: Option<String>) -> String {
    value
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|n| n.is_finite() && *n > 0.0)
        .map(|n| {
            if n < 1_000_000.0 {
                format!("{:.1} kb/s", n / 1000.0)
            } else {
                format!("{:.2} Mb/s", n / 1_000_000.0)
            }
        })
        .unwrap_or_else(|| "No disponible".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bitrates_keep_small_streams_readable_and_reject_nonfinite_values() {
        assert_eq!(bitrate(Some("1400".into())), "1.4 kb/s");
        assert_eq!(bitrate(Some("12000000".into())), "12.00 Mb/s");
        assert_eq!(bitrate(Some("NaN".into())), "No disponible");
        assert_eq!(bitrate(Some("-4".into())), "No disponible");
    }
    #[test]
    fn metadata_is_only_used_for_mkv_and_empty_titles_fall_back() {
        assert_eq!(
            title(r"F:\Movie.mkv", r#"{"TITLE":"Película"}"#),
            "Película"
        );
        assert_eq!(title(r"F:\Movie.mp4", r#"{"title":"Otro"}"#), "Movie.mp4");
        assert_eq!(title("/film.mkv", r#"{"title":" \n "}"#), "film.mkv");
        assert_eq!(title("/film.mkv", "broken"), "film.mkv");
        assert_eq!(
            title("/film.mkv", r#"{"title":"{\\an1} <&>"}"#),
            "{\\an1} <&>"
        );
    }
}
