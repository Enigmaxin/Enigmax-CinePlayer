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
type Open =
    unsafe extern "C" fn(*const c_char, c_int, c_int, *mut Info, *mut c_char, c_int) -> *mut c_void;
type Titles = unsafe extern "C" fn(*const c_char, *mut Title, c_int, *mut c_char, c_int) -> c_int;
type Read = unsafe extern "C" fn(*mut c_void, *mut c_void, u64) -> i64;
type Seek = unsafe extern "C" fn(*mut c_void, i64) -> i64;
type Size = unsafe extern "C" fn(*mut c_void) -> i64;
type Close = unsafe extern "C" fn(*mut c_void);
type Error = unsafe extern "C" fn(*mut c_void) -> *const c_char;
struct Api {
    _library: Library,
    open: Open,
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
            Ok(Arc::new(Self {
                open: std::mem::transmute::<*mut c_void, Open>(l.symbol(c"ec_mvc_open")?),
                titles: std::mem::transmute::<*mut c_void, Titles>(l.symbol(c"ec_disc_titles")?),
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
struct Engine {
    api: Arc<Api>,
    context: *mut c_void,
    error: Arc<Mutex<String>>,
}
// Ownership moves to mpv's demux thread. Only cancel runs concurrently; its C
// implementation touches an atomic flag, and mpv joins callbacks before close.
unsafe impl Send for Engine {}
impl Engine {
    fn failure(&self) {
        let p = unsafe { (self.api.error)(self.context) };
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
        unsafe { (self.api.close)(self.context) }
    }
}
pub struct Registry {
    api: Mutex<Option<Arc<Api>>>,
    pending: Mutex<Option<(String, Engine)>>,
    error: Arc<Mutex<String>>,
    library: std::path::PathBuf,
}
impl Registry {
    pub fn new(path: &Path) -> Self {
        Self {
            api: Mutex::new(None),
            pending: Mutex::new(None),
            error: Arc::new(Mutex::new(String::new())),
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
        let api = self.api()?;
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
            )
        };
        if n < 1 {
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
    pub fn prepare(&self, path: &str, title: i32, uri: &str) -> Result<Info, String> {
        let api = self.api()?;
        let path = CString::new(path).map_err(|_| "Ruta con NUL")?;
        let mut info = Info::default();
        let mut err = [0i8; 1024];
        let context = unsafe {
            (api.open)(
                path.as_ptr(),
                title,
                4,
                &mut info,
                err.as_mut_ptr(),
                err.len() as i32,
            )
        };
        if context.is_null() {
            return Err(unsafe { CStr::from_ptr(err.as_ptr()) }
                .to_string_lossy()
                .into_owned());
        }
        let engine = Engine {
            api,
            context,
            error: self.error.clone(),
        };
        if info.paired != 1
            || info.width <= 0
            || info.height <= 0
            || info.fps_num <= 0
            || info.fps_den <= 0
        {
            return Err("El decodificador no entregó dos vistas MVC válidas".into());
        }
        *self.error.lock().map_err(|_| "MVC: error bloqueado")? = String::new();
        *self.pending.lock().map_err(|_| "MVC: estado bloqueado")? = Some((uri.into(), engine));
        Ok(info)
    }
    pub fn error(&self) -> String {
        self.error
            .lock()
            .map(|e| e.clone())
            .unwrap_or_else(|_| "MVC: estado bloqueado".into())
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
    let r = unsafe { (e.api.read)(e.context, buf.cast(), n) };
    if r < 0 {
        e.failure();
    }
    r
}
unsafe extern "C" fn seek(cookie: *mut c_void, pos: i64) -> i64 {
    let e = unsafe { &*cookie.cast::<Engine>() };
    let r = unsafe { (e.api.seek)(e.context, pos) };
    if r < 0 {
        e.failure();
    }
    r
}
unsafe extern "C" fn size(cookie: *mut c_void) -> i64 {
    let e = unsafe { &*cookie.cast::<Engine>() };
    unsafe { (e.api.size)(e.context) }
}
unsafe extern "C" fn close(cookie: *mut c_void) {
    drop(unsafe { Box::from_raw(cookie.cast::<Engine>()) });
}
unsafe extern "C" fn cancel(cookie: *mut c_void) {
    let e = unsafe { &*cookie.cast::<Engine>() };
    unsafe { (e.api.cancel)(e.context) }
}
