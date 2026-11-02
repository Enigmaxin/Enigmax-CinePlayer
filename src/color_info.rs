//! Color information from decoder metadata, independent of names and presentation mode.
use serde_json::Value;
fn text(v: &Value, key: &str) -> String {
    v[key]
        .as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect()
}
fn positive(v: &Value, key: &str) -> bool {
    v[key].as_f64().is_some_and(|n| n.is_finite() && n > 0.0)
}
fn present(v: &Value, key: &str) -> bool {
    v[key].as_f64().is_some_and(|n| n.is_finite() && n >= 0.0)
}
pub fn describe(decoded: &Value, tracks: &Value) -> Vec<String> {
    let transfer = text(decoded, "gamma");
    let primaries = text(decoded, "primaries");
    let dv = tracks
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["type"] == "video" && t["selected"].as_bool() == Some(true))
        .find_map(|t| {
            t["dolby-vision-profile"]
                .as_u64()
                .map(|p| (p, t["dolby-vision-level"].as_u64()))
        });
    let dynamic = ["scene-max-r", "scene-max-g", "scene-max-b", "scene-avg"]
        .iter()
        .any(|k| present(decoded, k));
    let hdr10 = ["min-luma", "max-luma", "max-cll", "max-fall"]
        .iter()
        .any(|k| positive(decoded, k));
    let kind = if let Some((profile, level)) = dv {
        format!(
            "Dolby Vision · perfil {profile}{}{}",
            level.map(|l| format!(" · nivel {l}")).unwrap_or_default(),
            if dynamic { " · HDR10+ detectado" } else { "" }
        )
    } else if dynamic && transfer == "pq" {
        "HDR10+ · metadatos dinámicos detectados".into()
    } else if transfer == "pq" && primaries == "bt.2020" && hdr10 {
        "HDR10 · PQ / BT.2020".into()
    } else if transfer == "pq" {
        "HDR · PQ (HDR10+ / Dolby Vision no confirmados)".into()
    } else if transfer == "hlg" {
        "HDR · HLG".into()
    } else if transfer.is_empty() || transfer == "auto" || transfer == "unknown" {
        "Formato de color no disponible".into()
    } else if matches!(
        transfer.as_str(),
        "bt.1886"
            | "srgb"
            | "gamma1.8"
            | "gamma2.0"
            | "gamma2.2"
            | "gamma2.4"
            | "gamma2.6"
            | "gamma2.8"
            | "bt.709"
    ) {
        "SDR".into()
    } else {
        format!("Transferencia {transfer} · clasificación no confirmada")
    };
    let shown = |s: String| {
        if s.is_empty() {
            "No disponible".into()
        } else {
            s
        }
    };
    let mut rows = vec![
        format!("COLOR · {kind}"),
        format!(
            "Origen: {} · transferencia {} · matriz {} · rango {}",
            shown(primaries),
            shown(transfer),
            shown(text(decoded, "colormatrix")),
            shown(text(decoded, "colorlevels"))
        ),
    ];
    let metadata = [
        ("max-luma", "Mastering máx."),
        ("max-cll", "MaxCLL"),
        ("max-fall", "MaxFALL"),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        decoded[key]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| format!("{label} {n:.0} nits"))
    })
    .collect::<Vec<_>>();
    if !metadata.is_empty() {
        rows.push(metadata.join(" · "));
    }
    rows
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn source_metadata_distinguishes_sdr_pq_hlg_and_dynamic_hdr() {
        assert_eq!(
            describe(&json!({"gamma":"bt.1886","primaries":"bt.709"}), &json!([]))[0],
            "COLOR · SDR"
        );
        assert!(
            describe(
                &json!({"gamma":"pq","primaries":"bt.2020","max-luma":1000}),
                &json!([])
            )[0]
            .contains("HDR10 ·")
        );
        assert!(describe(&json!({"gamma":"pq","scene-max-r":0}), &json!([]))[0].contains("HDR10+"));
        assert!(describe(&json!({"gamma":"hlg"}), &json!([]))[0].contains("HLG"));
        assert!(
            !describe(
                &json!({"gamma":"bt.1886","max-luma":1000,"scene-max-r":100}),
                &json!([])
            )[0]
            .contains("HDR")
        );
    }
    #[test]
    fn missing_values_and_unselected_tracks_do_not_invent_dolby_vision() {
        let t = json!([{"type":"video","selected":false,"dolby-vision-profile":7},{"type":"video","selected":true,"title":"Dolby Vision HDR10+"}]);
        assert_eq!(
            describe(&Value::Null, &t)[0],
            "COLOR · Formato de color no disponible"
        );
        let dv = json!([{"type":"video","selected":true,"dolby-vision-profile":7,"dolby-vision-level":6}]);
        assert!(
            describe(&json!({"gamma":"pq"}), &dv)[0].contains("Dolby Vision · perfil 7 · nivel 6")
        );
        assert!(describe(&json!({"gamma":"pq"}), &json!([]))[0].contains("no confirmados"));
    }
    #[test]
    fn malformed_and_unknown_metadata_never_gets_a_guessed_hdr_label() {
        for value in [
            json!(null),
            json!(true),
            json!([]),
            json!({}),
            json!({"gamma":42}),
            json!({"gamma":"unrecognised"}),
        ] {
            for tracks in [
                json!(null),
                json!(false),
                json!({}),
                json!([null, 42, "Dolby Vision"]),
            ] {
                let rows = describe(&value, &tracks);
                assert!(!rows[0].contains("COLOR · SDR"));
                assert!(!rows[0].contains("COLOR · HDR10"));
                assert!(!rows[0].contains("COLOR · Dolby Vision"));
            }
        }
        let pq = json!({"gamma":"pq","primaries":"bt.2020","max-luma":-1,"scene-max-r":"1000"});
        assert!(describe(&pq, &json!([]))[0].contains("no confirmados"));
    }
}
