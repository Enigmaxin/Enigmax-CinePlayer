//! Presentation of reported properties only; absent values never become facts.
use serde_json::Value;
use std::collections::BTreeMap;

pub fn known(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "auto"
                | "unknown"
                | "unrecognized"
                | "unrecognised"
                | "undefined"
                | "n/a"
                | "none"
                | "no disponible"
                | "sin especificar"
        ))
    .then(|| {
        value
            .chars()
            .filter(|c| !c.is_control())
            .take(240)
            .collect()
    })
}

pub fn video(properties: &BTreeMap<String, String>, tracks: &Value) -> Vec<String> {
    let get = |key: &str| properties.get(key).and_then(|v| known(v));
    let positive = |key: &str| {
        get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let mut rows = Vec::new();
    let mut summary = Vec::new();
    if let Some(codec) = get("video-format") {
        summary.push(codec);
    }
    if let (Some(w), Some(h)) = (
        positive("video-dec-params/w"),
        positive("video-dec-params/h"),
    ) {
        summary.push(format!("{w:.0}×{h:.0}"));
    }
    if let Some(fps) = positive("container-fps") {
        summary.push(format!(
            "{} fps",
            format!("{fps:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
        ));
    }
    if let Some(rate) = positive("video-bitrate") {
        summary.push(crate::media::bitrate(Some(rate.to_string())));
    }
    if !summary.is_empty() {
        rows.push(summary.join(" · "));
    }
    let decoded: Value = properties
        .get("video-dec-params")
        .and_then(|v| serde_json::from_str(v).ok())
        .unwrap_or_default();
    let color = crate::color_info::describe(&decoded, tracks);
    if let Some(kind) = color
        .first()
        .filter(|s| !s.contains("no disponible") && !s.contains("no confirmada"))
    {
        // Scene luminance fields also occur with Dolby Vision. They do not
        // independently prove that an HDR10+ stream was recognized.
        let kind = kind
            .replace(" · HDR10+ detectado", "")
            .replace(
                "HDR10+ · metadatos dinámicos detectados",
                "HDR · PQ · metadatos dinámicos",
            )
            .replace(" (HDR10+ / Dolby Vision no confirmados)", "");
        rows.push(kind);
    }
    let color_fields: Vec<_> = [
        ("primaries", "Primarias"),
        ("gamma", "Transferencia"),
        ("colormatrix", "Matriz"),
        ("colorlevels", "Rango"),
    ]
    .into_iter()
    .filter_map(|(key, label)| known(decoded[key].as_str()?).map(|v| format!("{label}: {v}")))
    .collect();
    if !color_fields.is_empty() {
        rows.push(color_fields.join(" · "));
    }
    rows.extend(color.into_iter().skip(2));
    if let Some(pixels) =
        get("video-dec-params/hw-pixelformat").or_else(|| get("video-dec-params/pixelformat"))
    {
        rows.push(format!("Píxeles: {pixels}"));
    }
    let mut runtime = Vec::new();
    if let Some(decoder) = get("hwdec-current") {
        runtime.push(format!(
            "Decodificación: {}",
            if decoder == "no" {
                "software"
            } else {
                &decoder
            }
        ));
    }
    if let Some(renderer) = get("current-vo") {
        runtime.push(format!("Renderizador: {renderer}"));
    }
    if !runtime.is_empty() {
        rows.push(runtime.join(" · "));
    }
    if let Some(drops) = get("frame-drop-count").and_then(|v| v.parse::<u64>().ok()) {
        rows.push(format!("Fotogramas descartados: {drops}"));
    }
    for (key, label) in [
        ("video-params/gamma", "Transferencia tras filtros"),
        ("video-target-params/gamma", "Transferencia de salida"),
        ("video-target-params/primaries", "Primarias de salida"),
    ] {
        if let Some(value) = get(key) {
            rows.push(format!("{label}: {value}"));
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn absent_automatic_and_malformed_properties_are_not_shown() {
        let p = BTreeMap::from([
            ("video-format".into(), "auto".into()),
            ("video-bitrate".into(), "NaN".into()),
            ("container-fps".into(), "-1".into()),
            ("hwdec-current".into(), "unknown".into()),
            (
                "video-dec-params".into(),
                json!({"gamma":"auto","primaries":"unknown"}).to_string(),
            ),
        ]);
        assert!(video(&p, &json!([])).is_empty());
        assert!(video(&BTreeMap::new(), &Value::Null).is_empty());
    }
    #[test]
    fn reported_source_and_runtime_are_kept_without_claiming_output_hdr() {
        let p = BTreeMap::from([
            ("video-format".into(), "hevc".into()),
            ("video-dec-params/w".into(), "3840".into()),
            ("video-dec-params/h".into(), "2160".into()),
            ("container-fps".into(), "23.976023976".into()),
            ("hwdec-current".into(), "no".into()),
            ("frame-drop-count".into(), "0".into()),
            (
                "video-dec-params".into(),
                json!({"gamma":"pq","primaries":"bt.2020","max-luma":1000}).to_string(),
            ),
        ]);
        let rows = video(&p, &json!([])).join("\n");
        assert!(rows.contains("3840×2160 · 23.976 fps"));
        assert!(rows.contains("COLOR · HDR10"));
        assert!(rows.contains("Decodificación: software"));
        assert!(rows.contains("descartados: 0"));
        assert!(!rows.contains("salida"));
        assert!(!rows.contains("no confirmados"));
    }
    #[test]
    fn dynamic_scene_values_do_not_add_a_second_unconfirmed_hdr_format() {
        let p = BTreeMap::from([(
            "video-dec-params".into(),
            json!({"gamma":"pq","scene-max-r":100}).to_string(),
        )]);
        let tracks = json!([{"type":"video","selected":true,"dolby-vision-profile":7}]);
        let rows = video(&p, &tracks).join("\n");
        assert!(rows.contains("Dolby Vision · perfil 7"));
        assert!(!rows.contains("HDR10+"));
    }
}
