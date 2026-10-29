//! A single bounded structural probe worker. Never decodes pixels or blocks UI.
use crate::windows::Library;
use std::{
    ffi::{CString, c_char, c_int, c_void},
    path::Path,
    sync::mpsc::{self, Receiver, SyncSender},
    time::SystemTime,
};
type Probe = unsafe extern "C" fn(*const c_char) -> c_int;
#[derive(Clone, PartialEq, Eq)]
struct Key {
    path: String,
    bytes: u64,
    modified: Option<SystemTime>,
}
impl Key {
    fn new(path: &str) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        meta.is_file().then(|| Self {
            path: path.into(),
            bytes: meta.len(),
            modified: meta.modified().ok(),
        })
    }
}
pub struct Worker {
    tx: SyncSender<Key>,
    rx: Receiver<(Key, bool)>,
    pending: Option<Key>,
    active: Option<Key>,
    pub mvc: bool,
    processing: usize,
}
impl Worker {
    pub fn new(dir: &Path) -> Option<Self> {
        let lib = dir.join("runtime/cine_mvc.dll");
        if !lib.is_file() {
            return None;
        }
        let (tx, jobs) = mpsc::sync_channel::<Key>(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("cine-structural-probe".into())
            .spawn(move || {
                let Ok(library) = Library::load(&lib) else {
                    return;
                };
                let Ok(symbol) = library.symbol(c"ec_probe_mvc") else {
                    return;
                };
                // The module stays loaded for the lifetime of the worker.
                let probe: Probe = unsafe { std::mem::transmute::<*mut c_void, Probe>(symbol) };
                let mut cache = Vec::<(Key, bool)>::new();
                while let Ok(key) = jobs.recv() {
                    let mvc = cache
                        .iter()
                        .find(|(k, _)| k == &key)
                        .map(|(_, v)| *v)
                        .unwrap_or_else(|| {
                            let result = CString::new(key.path.as_str())
                                .ok()
                                .is_some_and(|p| unsafe { probe(p.as_ptr()) } == 1);
                            if cache.len() >= 32 {
                                cache.remove(0);
                            }
                            cache.push((key.clone(), result));
                            result
                        });
                    if results.send((key, mvc)).is_err() {
                        break;
                    }
                }
            })
            .ok()?;
        Some(Self {
            tx,
            rx,
            pending: None,
            active: None,
            mvc: false,
            processing: 0,
        })
    }
    pub fn request(&mut self, path: &str) {
        let key = Key::new(path);
        if self.active == key {
            return;
        }
        self.mvc = false;
        self.active = key;
        self.pending = self.active.clone();
    }
    pub fn waiting(&self) -> bool {
        self.processing > 0 || self.pending.is_some()
    }
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.rx.try_recv() {
                Ok((key, mvc)) => {
                    self.processing = self.processing.saturating_sub(1);
                    if self.active.as_ref() == Some(&key) {
                        changed |= self.mvc != mvc;
                        self.mvc = mvc;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.processing = 0;
                    self.pending = None;
                    return changed;
                }
            }
        }
        if let Some(key) = self.pending.take() {
            match self.tx.try_send(key) {
                Ok(()) => self.processing += 1,
                Err(mpsc::TrySendError::Full(k)) => self.pending = Some(k),
                Err(_) => {}
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_results_are_ignored_and_changed_files_invalidate_the_cache_key() {
        let temp = tempfile::tempdir().unwrap();
        let a = temp.path().join("a.mkv");
        let b = temp.path().join("b.mkv");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"b").unwrap();
        let (tx, _jobs) = mpsc::sync_channel(1);
        let (results, rx) = mpsc::sync_channel(2);
        let mut worker = Worker {
            tx,
            rx,
            pending: None,
            active: None,
            mvc: false,
            processing: 0,
        };
        worker.request(a.to_str().unwrap());
        let old = worker.active.clone().unwrap();
        worker.request(b.to_str().unwrap());
        results.send((old, true)).unwrap();
        assert!(!worker.poll());
        assert!(!worker.mvc);
        results
            .send((worker.active.clone().unwrap(), true))
            .unwrap();
        assert!(worker.poll());
        assert!(worker.mvc);
        worker.request(b.to_str().unwrap());
        assert!(worker.mvc);
        std::fs::write(&b, b"modified").unwrap();
        worker.request(b.to_str().unwrap());
        assert!(!worker.mvc);
    }
}
