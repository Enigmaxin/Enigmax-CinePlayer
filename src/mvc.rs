//! Read-only, bounded MVC transport. Callbacks never call libmpv.
use crate::windows::Library;
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::Path,
    sync::{Arc, Mutex},
};

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Info {
    pub width: i32,
    pub height: i32,
    pub format: i32,
    pub fps_num: i32,
    pub fps_den: i32,
    pub playlist: i32,
    pub clips: i32,
    pub paired: i32,
    pub frames: i64,
    pub duration: f64,
    pub audio_delay: f64,
    pub metadata: Metadata,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Metadata {
    pub sar_num: i32,
    pub sar_den: i32,
    pub range: i32,
    pub matrix: i32,
    pub primaries: i32,
    pub transfer: i32,
    pub chroma: i32,
    pub has_audio: i32,
    pub has_sub: i32,
}
impl Metadata {
    pub fn pixel_aspect(self) -> f64 {
        if self.sar_num > 0 && self.sar_den > 0 {
            self.sar_num as f64 / self.sar_den as f64
        } else {
            1.0
        }
    }
    pub fn filter(self) -> String {
        let mut args = Vec::new();
        let matrix = match self.matrix {
            1 => "bt.709",
            5 | 6 => "bt.601",
            7 => "smpte-240m",
            9 => "bt.2020-ncl",
            10 => "bt.2020-cl",
            _ => "",
        };
        let range = match self.range {
            1 => "limited",
            2 => "full",
            _ => "",
        };
        let primaries = match self.primaries {
            1 => "bt.709",
            4 => "bt.470m",
            5 => "bt.601-625",
            6 => "bt.601-525",
            7 => "bt.601-525",
            9 => "bt.2020",
            _ => "",
        };
        let transfer = match self.transfer {
            1 | 6 | 14 | 15 => "bt.1886",
            4 => "gamma2.2",
            5 => "gamma2.8",
            7 => "bt.1886",
            8 => "linear",
            13 => "srgb",
            16 => "pq",
            18 => "hlg",
            _ => "",
        };
        let chroma = match self.chroma {
            1 => "mpeg2/4/h264",
            2 => "mpeg1/jpeg",
            3 => "uhd",
            _ => "",
        };
        for (key, value) in [
            ("colormatrix", matrix),
            ("colorlevels", range),
            ("primaries", primaries),
            ("gamma", transfer),
            ("chroma-location", chroma),
        ] {
            if !value.is_empty() {
                args.push(format!("{key}={value}"));
            }
        }
        if args.is_empty() {
            String::new()
        } else {
            format!("format={}", args.join(":"))
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct Title {
    pub playlist: u32,
    pub clips: u32,
    pub duration: f64,
    pub stereo: i32,
    pub repeated: i32,
}
type Open = unsafe extern "C" fn(
    *const c_char,
    c_int,
    c_int,
    *mut Info,
    *mut c_char,
    c_int,
    *mut c_void,
) -> *mut c_void;
type Titles = unsafe extern "C" fn(
    *const c_char,
    *mut Title,
    c_int,
    *mut c_char,
    c_int,
    *mut c_void,
) -> c_int;
type Read = unsafe extern "C" fn(*mut c_void, *mut c_void, u64) -> i64;
type Seek = unsafe extern "C" fn(*mut c_void, i64) -> i64;
type Size = unsafe extern "C" fn(*mut c_void) -> i64;
type Close = unsafe extern "C" fn(*mut c_void);
type Error = unsafe extern "C" fn(*mut c_void) -> *const c_char;
struct Api {
    _library: Library,
    cancel_create: unsafe extern "C" fn() -> *mut c_void,
    cancel_set: Close,
    cancel_free: Close,
    metadata: unsafe extern "C" fn(*mut c_void, *mut Metadata) -> c_int,
    open: Open,
    disc_open: Open,
    disc_io: Io,
    titles: Titles,
    read: Read,
    seek: Seek,
    size: Size,
    close: Close,
    cancel: Close,
    error: Error,
}
// Module references and immutable function pointers can be used from any thread;
// the final reference unloads the library only after every Engine has closed.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}
impl Api {
    fn load(path: &Path) -> Result<Arc<Self>, String> {
        let l = Library::load(path)?;
        // SAFETY: native/cine_mvc.h is built and shipped with this application.
        unsafe {
            let version: unsafe extern "C" fn() -> c_int =
                std::mem::transmute(l.symbol(c"ec_bridge_abi_version")?);
            if version() != 1 {
                return Err("Puente MVC incompatible: reinstala el paquete completo".into());
            }
            Ok(Arc::new(Self {
                cancel_create: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn() -> *mut c_void,
                >(l.symbol(c"ec_cancel_create")?),
                cancel_set: std::mem::transmute::<*mut c_void, Close>(l.symbol(c"ec_cancel_set")?),
                cancel_free: std::mem::transmute::<*mut c_void, Close>(
                    l.symbol(c"ec_cancel_free")?,
                ),
                metadata: std::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn(*mut c_void, *mut Metadata) -> c_int,
                >(l.symbol(c"ec_mvc_metadata")?),
                open: std::mem::transmute::<*mut c_void, Open>(l.symbol(c"ec_mvc_open_ex")?),
                disc_open: std::mem::transmute::<*mut c_void, Open>(
                    l.symbol(c"ec_disc_stream_open_ex")?,
                ),
                disc_io: Io {
                    read: std::mem::transmute::<*mut c_void, Read>(
                        l.symbol(c"ec_disc_stream_read")?,
                    ),
                    seek: std::mem::transmute::<*mut c_void, Seek>(
                        l.symbol(c"ec_disc_stream_seek")?,
                    ),
                    size: std::mem::transmute::<*mut c_void, Size>(
                        l.symbol(c"ec_disc_stream_size")?,
                    ),
                    close: std::mem::transmute::<*mut c_void, Close>(
                        l.symbol(c"ec_disc_stream_close")?,
                    ),
                    cancel: std::mem::transmute::<*mut c_void, Close>(
                        l.symbol(c"ec_disc_stream_cancel")?,
                    ),
                    error: std::mem::transmute::<*mut c_void, Error>(
                        l.symbol(c"ec_disc_stream_error")?,
                    ),
                },
                titles: std::mem::transmute::<*mut c_void, Titles>(l.symbol(c"ec_disc_titles_ex")?),
                read: std::mem::transmute::<*mut c_void, Read>(l.symbol(c"ec_mvc_read")?),
                seek: std::mem::transmute::<*mut c_void, Seek>(l.symbol(c"ec_mvc_seek")?),
                size: std::mem::transmute::<*mut c_void, Size>(l.symbol(c"ec_mvc_size")?),
                close: std::mem::transmute::<*mut c_void, Close>(l.symbol(c"ec_mvc_close")?),
                cancel: std::mem::transmute::<*mut c_void, Close>(l.symbol(c"ec_mvc_cancel")?),
                error: std::mem::transmute::<*mut c_void, Error>(l.symbol(c"ec_mvc_error")?),
                _library: l,
            }))
        }
    }
}
#[derive(Clone, Copy)]
struct Io {
    read: Read,
    seek: Seek,
    size: Size,
    close: Close,
    cancel: Close,
    error: Error,
}
struct Engine {
    _api: Arc<Api>,
    context: *mut c_void,
    _stop: Option<Arc<Cancellation>>,
    io: Io,
    error: Arc<Mutex<String>>,
}
// Ownership moves to mpv's demux thread. Only cancel runs concurrently; its C
// implementation touches an atomic flag, and mpv joins callbacks before close.
unsafe impl Send for Engine {}
impl Engine {
    fn failure(&self) {
        let p = unsafe { (self.io.error)(self.context) };
        if !p.is_null() {
            let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
            if let Ok(mut e) = self.error.lock() {
                *e = s;
            }
        }
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        unsafe { (self.io.close)(self.context) }
    }
}
pub struct Registry {
    api: Mutex<Option<Arc<Api>>>,
    pending: Mutex<Option<(String, Engine)>>,
    error: Mutex<Arc<Mutex<String>>>,
    library: std::path::PathBuf,
}
impl Registry {
    pub fn new(path: &Path) -> Self {
        Self {
            api: Mutex::new(None),
            pending: Mutex::new(None),
            error: Mutex::new(Arc::new(Mutex::new(String::new()))),
            library: path.into(),
        }
    }
    fn api(&self) -> Result<Arc<Api>, String> {
        let mut guard = self.api.lock().map_err(|_| "MVC: estado bloqueado")?;
        if let Some(a) = &*guard {
            return Ok(a.clone());
        }
        let api = Api::load(&self.library)?;
        *guard = Some(api.clone());
        Ok(api)
    }
    pub fn titles(&self, path: &str) -> Result<Vec<Title>, String> {
        titles(self.api()?, path, None)
    }
    pub fn loader(&self) -> Result<Loader, String> {
        Loader::new(self.api()?)
    }
    pub fn adopt(&self, uri: &str, mut prepared: Prepared) -> Result<Info, String> {
        // A cancelled old stream must never overwrite the current stream's diagnostic.
        let error = Arc::new(Mutex::new(String::new()));
        prepared.engine.error = error.clone();
        *self.error.lock().map_err(|_| "MVC: error bloqueado")? = error;
        let info = prepared.info;
        *self.pending.lock().map_err(|_| "MVC: estado bloqueado")? =
            Some((uri.into(), prepared.engine));
        Ok(info)
    }
    pub fn prepare(&self, path: &str, title: i32, uri: &str) -> Result<Info, String> {
        self.adopt(uri, prepare(self.api()?, Kind::Mvc, path, title, None)?)
    }
    pub fn prepare_disc(&self, path: &str, title: i32, uri: &str) -> Result<Info, String> {
        self.adopt(uri, prepare(self.api()?, Kind::Disc, path, title, None)?)
    }
    pub fn error(&self) -> String {
        self.error
            .lock()
            .ok()
            .and_then(|current| current.lock().ok().map(|e| e.clone()))
            .unwrap_or_else(|| "MVC: estado bloqueado".into())
    }
}
// Cancellation storage is allocated by C: no cross-language Atomic layout assumption.
struct Cancellation {
    api: Arc<Api>,
    ptr: *mut c_void,
}
unsafe impl Send for Cancellation {}
unsafe impl Sync for Cancellation {}
impl Cancellation {
    fn new(api: Arc<Api>) -> Result<Arc<Self>, String> {
        let ptr = unsafe { (api.cancel_create)() };
        if ptr.is_null() {
            return Err("Sin memoria para cancelación".into());
        }
        Ok(Arc::new(Self { api, ptr }))
    }
    fn cancel(&self) {
        unsafe { (self.api.cancel_set)(self.ptr) }
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        unsafe { (self.api.cancel_free)(self.ptr) }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Titles,
    Mvc,
    Disc,
}
pub struct Prepared {
    pub info: Info,
    engine: Engine,
}
pub enum Completion {
    Titles(Vec<Title>),
    Stream(Prepared),
}
fn titles(api: Arc<Api>, path: &str, stop: Option<&Cancellation>) -> Result<Vec<Title>, String> {
    let path = CString::new(path).map_err(|_| "Ruta con NUL")?;
    let mut values = vec![Title::default(); 4096];
    let mut err = [0i8; 1024];
    let n = unsafe {
        (api.titles)(
            path.as_ptr(),
            values.as_mut_ptr(),
            values.len() as i32,
            err.as_mut_ptr(),
            err.len() as i32,
            stop.map_or(std::ptr::null_mut(), |s| s.ptr),
        )
    };
    if n < 1 || n as usize > values.len() {
        return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
            .to_string_lossy()
            .into_owned());
    }
    values.truncate(n as usize);
    values.sort_by(|a, b| {
        a.repeated
            .cmp(&b.repeated)
            .then_with(|| b.duration.total_cmp(&a.duration))
    });
    Ok(values)
}
fn prepare(
    api: Arc<Api>,
    kind: Kind,
    path: &str,
    title: i32,
    stop: Option<Arc<Cancellation>>,
) -> Result<Prepared, String> {
    let path = CString::new(path).map_err(|_| "Ruta con NUL")?;
    let mut info = Info::default();
    let mut err = [0i8; 1024];
    let disc = kind == Kind::Disc;
    let context = unsafe {
        (if disc { api.disc_open } else { api.open })(
            path.as_ptr(),
            title,
            if disc {
                0
            } else {
                cineplayer_core::performance::mvc_threads(
                    std::thread::available_parallelism().map_or(1, usize::from),
                ) as i32
            },
            &mut info,
            err.as_mut_ptr(),
            err.len() as i32,
            stop.as_ref().map_or(std::ptr::null_mut(), |s| s.ptr),
        )
    };
    if context.is_null() {
        return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
            .to_string_lossy()
            .into_owned());
    }
    let engine = Engine {
        io: if disc {
            api.disc_io
        } else {
            Io {
                read: api.read,
                seek: api.seek,
                size: api.size,
                close: api.close,
                cancel: api.cancel,
                error: api.error,
            }
        },
        context,
        _stop: stop,
        error: Arc::new(Mutex::new(String::new())),
        _api: api.clone(),
    };
    if !disc {
        if info.paired != 1
            || info.width <= 0
            || info.height <= 0
            || info.fps_num <= 0
            || info.fps_den <= 0
        {
            return Err("El decodificador no entregó dos vistas MVC válidas".into());
        }
        if unsafe { (api.metadata)(context, &mut info.metadata) } < 0 {
            return Err("No se recuperaron metadatos MVC".into());
        }
    }
    Ok(Prepared { info, engine })
}
struct Job {
    id: u64,
    kind: Kind,
    path: String,
    title: i32,
    stop: Arc<Cancellation>,
}
struct Slot {
    job: Option<Job>,
    shutdown: bool,
}
pub struct Loader {
    api: Arc<Api>,
    slot: Arc<(Mutex<Slot>, std::sync::Condvar)>,
    rx: std::sync::mpsc::Receiver<(u64, Result<Completion, String>)>,
    stop: Option<Arc<Cancellation>>,
    id: u64,
    worker: Option<std::thread::JoinHandle<()>>,
    disconnected: bool,
}
impl Loader {
    fn new(api: Arc<Api>) -> Result<Self, String> {
        let slot = Arc::new((
            Mutex::new(Slot {
                job: None,
                shutdown: false,
            }),
            std::sync::Condvar::new(),
        ));
        let jobs = slot.clone();
        let engine = api.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("cine-media-loader".into())
            .spawn(move || {
                loop {
                    let job = {
                        let (guard, cv) = (&jobs.0, &jobs.1);
                        let Ok(mut slot) = guard.lock() else { return };
                        while slot.job.is_none() && !slot.shutdown {
                            slot = match cv.wait(slot) {
                                Ok(s) => s,
                                Err(_) => return,
                            };
                        }
                        if slot.shutdown {
                            return;
                        }
                        slot.job.take().unwrap()
                    };
                    let result = if job.kind == Kind::Titles {
                        titles(engine.clone(), &job.path, Some(&job.stop)).map(Completion::Titles)
                    } else {
                        prepare(
                            engine.clone(),
                            job.kind,
                            &job.path,
                            job.title,
                            Some(job.stop.clone()),
                        )
                        .map(Completion::Stream)
                    };
                    if tx.send((job.id, result)).is_err() {
                        return;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            api,
            slot,
            rx,
            stop: None,
            id: 0,
            worker: Some(worker),
            disconnected: false,
        })
    }
    pub fn request(&mut self, kind: Kind, path: &str, title: i32) -> Result<u64, String> {
        if self.disconnected {
            return Err("El trabajador de medios no está disponible".into());
        }
        self.cancel();
        self.id = self.id.wrapping_add(1);
        let stop = Cancellation::new(self.api.clone())?;
        let job = Job {
            id: self.id,
            kind,
            path: path.into(),
            title,
            stop: stop.clone(),
        };
        self.slot.0.lock().map_err(|_| "Carga bloqueada")?.job = Some(job);
        self.stop = Some(stop);
        self.slot.1.notify_one();
        Ok(self.id)
    }
    pub fn completed(&mut self) {
        self.stop = None;
    }
    pub fn cancel(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.cancel();
        }
        if let Ok(mut slot) = self.slot.0.lock() {
            slot.job = None;
        }
    }
    pub fn poll(&mut self) -> Option<(u64, Result<Completion, String>)> {
        if self.disconnected {
            return None;
        }
        match self.rx.try_recv() {
            Ok(value) => Some(value),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.disconnected = true;
                Some((
                    self.id,
                    Err("El trabajador de medios terminó inesperadamente".into()),
                ))
            }
        }
    }
}
impl Drop for Loader {
    fn drop(&mut self) {
        self.cancel();
        if let Ok(mut slot) = self.slot.0.lock() {
            slot.shutdown = true;
        }
        self.slot.1.notify_one();
        // Never deadlock exit on a broken storage driver. The worker owns all
        // its resources and never calls mpv, even if kernel cancellation stalls.
        if let Some(worker) = self.worker.take() {
            let until = std::time::Instant::now() + std::time::Duration::from_millis(500);
            while !worker.is_finished() && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}
#[repr(C)]
pub struct StreamInfo {
    cookie: *mut c_void,
    read: Option<unsafe extern "C" fn(*mut c_void, *mut c_char, u64) -> i64>,
    seek: Option<unsafe extern "C" fn(*mut c_void, i64) -> i64>,
    size: Option<unsafe extern "C" fn(*mut c_void) -> i64>,
    close: Option<unsafe extern "C" fn(*mut c_void)>,
    cancel: Option<unsafe extern "C" fn(*mut c_void)>,
}
pub type OpenCallback = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut StreamInfo) -> c_int;
pub unsafe extern "C" fn open(user: *mut c_void, uri: *mut c_char, out: *mut StreamInfo) -> c_int {
    if user.is_null() || uri.is_null() || out.is_null() {
        return -13;
    }
    let registry = unsafe { &*user.cast::<Registry>() };
    let Ok(mut pending) = registry.pending.lock() else {
        return -13;
    };
    let requested = unsafe { CStr::from_ptr(uri) }.to_string_lossy();
    if pending
        .as_ref()
        .is_none_or(|(id, _)| id != requested.as_ref())
    {
        return -13;
    }
    let Some((_, engine)) = pending.take() else {
        return -13;
    };
    unsafe {
        *out = StreamInfo {
            cookie: Box::into_raw(Box::new(engine)).cast(),
            read: Some(read),
            seek: Some(seek),
            size: Some(size),
            close: Some(close),
            cancel: Some(cancel),
        }
    };
    0
}
unsafe extern "C" fn read(cookie: *mut c_void, buf: *mut c_char, n: u64) -> i64 {
    let e = unsafe { &*cookie.cast::<Engine>() };
    let r = unsafe { (e.io.read)(e.context, buf.cast(), n) };
    if r < 0 {
        e.failure();
    }
    r
}
unsafe extern "C" fn seek(cookie: *mut c_void, pos: i64) -> i64 {
    let e = unsafe { &*cookie.cast::<Engine>() };
    let r = unsafe { (e.io.seek)(e.context, pos) };
    if r < 0 {
        e.failure();
    }
    r
}
unsafe extern "C" fn size(cookie: *mut c_void) -> i64 {
    let e = unsafe { &*cookie.cast::<Engine>() };
    unsafe { (e.io.size)(e.context) }
}
unsafe extern "C" fn close(cookie: *mut c_void) {
    drop(unsafe { Box::from_raw(cookie.cast::<Engine>()) });
}
unsafe extern "C" fn cancel(cookie: *mut c_void) {
    let e = unsafe { &*cookie.cast::<Engine>() };
    unsafe { (e.io.cancel)(e.context) }
}
