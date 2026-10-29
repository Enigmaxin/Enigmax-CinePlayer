//! Immutable shader paths: both mpv renderers cache source by filename.
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
#[derive(Default)]
pub struct ShaderStore {
    directory: Option<tempfile::TempDir>,
    paths: HashMap<String, PathBuf>,
}
impl ShaderStore {
    pub fn path(&mut self, root: &Path, source: String) -> Result<PathBuf, String> {
        if let Some(path) = self.paths.get(&source) {
            return Ok(path.clone());
        }
        if self.directory.is_none() {
            let config = root.join("config");
            std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
            self.directory = Some(
                tempfile::Builder::new()
                    .prefix("session-shaders-")
                    .tempdir_in(config)
                    .map_err(|e| e.to_string())?,
            );
        }
        let path = self
            .directory
            .as_ref()
            .ok_or("Falta la carpeta de shaders")?
            .path()
            .join(format!("{}.glsl", self.paths.len()));
        std::fs::write(&path, &source).map_err(|e| e.to_string())?;
        self.paths.insert(source, path.clone());
        Ok(path)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_variants_are_reused_and_cleaned_without_cross_session_races() {
        let root = tempfile::tempdir().unwrap();
        let mut a = ShaderStore::default();
        let mut b = ShaderStore::default();
        let first = a.path(root.path(), "red/cyan".into()).unwrap();
        let second = a.path(root.path(), "green/magenta".into()).unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "red/cyan");
        assert_eq!(a.path(root.path(), "red/cyan".into()).unwrap(), first);
        let other = b.path(root.path(), "red/cyan".into()).unwrap();
        assert_ne!(first, other);
        drop(a);
        assert!(!first.exists());
        assert!(other.exists());
        drop(b);
        assert!(!other.exists());
    }
}
