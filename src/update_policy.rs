//! Pure release validation, shared with tests and future platform frontends.
use semver::Version;
use serde_json::Value;

pub const UPDATE_ENDPOINT: &str =
    "https://api.github.com/repos/Enigmaxin/Enigmax-CinePlayer/releases/latest";
pub const MAX_PACKAGE_BYTES: u64 = 512 * 1024 * 1024;
const DOWNLOAD_PREFIX: &str = "https://github.com/Enigmaxin/Enigmax-CinePlayer/releases/download/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}
pub fn version(tag: &str) -> Result<Version, String> {
    Version::parse(
        tag.strip_prefix('v')
            .or_else(|| tag.strip_prefix('V'))
            .unwrap_or(tag),
    )
    .map_err(|_| "Versión semántica no válida".into())
}
fn escape_segment(s: &str) -> String {
    let mut out = String::new();
    for c in s.bytes() {
        if c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.' | b'~') {
            out.push(c as char)
        } else {
            use std::fmt::Write;
            write!(out, "%{c:02X}").unwrap();
        }
    }
    out
}
pub fn latest(json: &[u8], current: &str) -> Result<Option<Release>, String> {
    let root: Value = serde_json::from_slice(json).map_err(|_| "GitHub devolvió JSON no válido")?;
    if root.get("draft").and_then(Value::as_bool) != Some(false)
        || root.get("prerelease").and_then(Value::as_bool) != Some(false)
    {
        return Ok(None);
    }
    let tag = root
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or("Falta tag_name")?;
    let remote = version(tag)?;
    if !remote.pre.is_empty() || !remote.cmp_precedence(&version(current)?).is_gt() {
        return Ok(None);
    }
    if !remote.build.is_empty() {
        return Err(
            "El paquete estable debe usar una versión mayor.menor.parche sin metadatos".into(),
        );
    }
    let name = format!("Enigmax-CinePlayer-Portable-{remote}-windows-x64.zip");
    let assets = root
        .get("assets")
        .and_then(Value::as_array)
        .ok_or("Falta la lista de assets")?;
    let matches = assets
        .iter()
        .filter(|a| a.get("name").and_then(Value::as_str) == Some(&name))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err("La release debe contener un único ZIP portátil x64 de Enigmax CinePlayer con el número de versión correcto".into());
    }
    let asset = matches[0];
    let url = asset
        .get("browser_download_url")
        .and_then(Value::as_str)
        .ok_or("Falta browser_download_url")?;
    let expected = format!(
        "{DOWNLOAD_PREFIX}{}/{}",
        escape_segment(tag),
        escape_segment(&name)
    );
    if url != expected {
        return Err("El paquete no pertenece a la release esperada".into());
    }
    let digest = asset
        .get("digest")
        .and_then(Value::as_str)
        .and_then(|s| s.strip_prefix("sha256:"))
        .ok_or("La release no incluye el SHA-256 del paquete")?;
    if digest.len() != 64 || !digest.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("SHA-256 no válido".into());
    }
    let size = asset
        .get("size")
        .and_then(Value::as_u64)
        .filter(|n| *n >= 4096 && *n <= MAX_PACKAGE_BYTES)
        .ok_or("Tamaño del paquete fuera de rango")?;
    Ok(Some(Release {
        version: remote,
        url: url.into(),
        size,
        sha256: digest.to_ascii_lowercase(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(tag: &str) -> Value {
        let v = version(tag).unwrap();
        let name = format!("Enigmax-CinePlayer-Portable-{v}-windows-x64.zip");
        serde_json::json!({"draft":false,"prerelease":false,"tag_name":tag,"assets":[{"name":name,"size":74000000,"digest":format!("sha256:{}","a".repeat(64)),"browser_download_url":format!("{DOWNLOAD_PREFIX}{tag}/{name}")}]})
    }
    #[test]
    fn semantic_order_and_no_downgrades() {
        assert!(version("v0.10.0").unwrap() > version("0.9.99").unwrap());
        assert!(version("1.0.0").unwrap() > version("1.0.0-rc.1").unwrap());
        assert!(version("01.0.0").is_err());
        let data = serde_json::to_vec(&fixture("v0.2.5")).unwrap();
        assert!(latest(&data, "0.2.4").unwrap().is_some());
        assert!(latest(&data, "0.2.5").unwrap().is_none());
        assert!(latest(&data, "0.10.0").unwrap().is_none());
        let data = serde_json::to_vec(&fixture("v0.2.5+build.2")).unwrap();
        assert!(latest(&data, "0.2.5+build.1").unwrap().is_none());
        assert!(latest(&data, "0.2.4").is_err());
    }
    #[test]
    fn unsafe_or_ambiguous_assets_are_rejected() {
        for field in ["browser_download_url", "digest", "size"] {
            let mut j = fixture("v0.2.5");
            j["assets"][0][field] = Value::Null;
            assert!(latest(&serde_json::to_vec(&j).unwrap(), "0.2.4").is_err());
        }
        let mut j = fixture("v0.2.5");
        j["assets"][0]["browser_download_url"] =
            Value::String("https://example.com/Enigmax.exe".into());
        assert!(latest(&serde_json::to_vec(&j).unwrap(), "0.2.4").is_err());
        let mut j = fixture("v0.2.5");
        let a = j["assets"][0].clone();
        j["assets"].as_array_mut().unwrap().push(a);
        assert!(latest(&serde_json::to_vec(&j).unwrap(), "0.2.4").is_err());
        let mut j = fixture("v0.2.5");
        j["prerelease"] = Value::Bool(true);
        assert!(
            latest(&serde_json::to_vec(&j).unwrap(), "0.2.4")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn installers_and_unbounded_or_invalid_payloads_are_rejected() {
        let mut j = fixture("v1.0.1");
        j["assets"][0]["name"] = "Enigmax-CinePlayer-Setup-1.0.1-x64.exe".into();
        assert!(latest(&serde_json::to_vec(&j).unwrap(), "1.0.0").is_err());
        for size in [0, 4095, MAX_PACKAGE_BYTES + 1] {
            let mut j = fixture("v1.0.1");
            j["assets"][0]["size"] = size.into();
            assert!(latest(&serde_json::to_vec(&j).unwrap(), "1.0.0").is_err());
        }
        for digest in [
            "sha256:abc",
            "md5:abc",
            &format!("sha256:{}", "z".repeat(64)),
        ] {
            let mut j = fixture("v1.0.1");
            j["assets"][0]["digest"] = digest.into();
            assert!(latest(&serde_json::to_vec(&j).unwrap(), "1.0.0").is_err());
        }
    }
}
