//! Minimal owned bindings to the stable libmpv C client API.
//! The library is loaded by absolute path, never from a media file's directory.
use crate::windows;
use std::{
    cell::RefCell,
    ffi::{CStr, CString, c_char, c_int, c_void},
    path::Path,
    ptr,
};

#[repr(C)]
pub struct Event {
    pub id: c_int,
    pub error: c_int,
    pub userdata: u64,
    pub data: *mut c_void,
}
#[repr(C)]
pub struct ClientMessage {
    pub count: c_int,
    pub args: *const *const c_char,
}
#[repr(C)]
pub struct EndFile {
    pub reason: c_int,
    pub error: c_int,
}

type Handle = *mut c_void;
type Create = unsafe extern "C" fn() -> Handle;
type Destroy = unsafe extern "C" fn(Handle);
type Init = unsafe extern "C" fn(Handle) -> c_int;
type Set = unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int;
type Command = unsafe extern "C" fn(Handle, *const *const c_char) -> c_int;
type CommandAsync = unsafe extern "C" fn(Handle, u64, *const *const c_char) -> c_int;
type Get = unsafe extern "C" fn(Handle, *const c_char) -> *mut c_char;
type Free = unsafe extern "C" fn(*mut c_void);
type Wait = unsafe extern "C" fn(Handle, f64) -> *const Event;
type Error = unsafe extern "C" fn(c_int) -> *const c_char;
type Name = unsafe extern "C" fn(Handle) -> *const c_char;
type Version = unsafe extern "C" fn() -> u64;
#[repr(C)]
union NodeValue {
    string: *mut c_char,
    list: *mut NodeList,
    number: i64,
    #[cfg(test)]
    bytes: *mut ByteArray,
}
#[cfg(test)]
#[repr(C)]
struct ByteArray {
    data: *mut u8,
    size: usize,
}
#[repr(C)]
struct Node {
    value: NodeValue,
    format: c_int,
}
#[repr(C)]
struct NodeList {
    count: c_int,
    values: *mut Node,
    keys: *mut *mut c_char,
}

pub struct Mpv {
    handle: Handle,
    library: windows::Library,
    destroy: Destroy,
    init: Init,
    option: Set,
    set: Set,
    command: Command,
    command_async: CommandAsync,
    seeks: RefCell<cineplayer_core::seek::Queue>,
    get: Get,
    free: Free,
    wait: Wait,
    error: Error,
    name: Name,
    registry: Option<Box<crate::mvc::Registry>>,
}

fn c(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| "Texto con NUL no permitido".into())
}

impl Mpv {
    pub fn popup_client(&self) -> Result<PopupClient<'_>, String> {
        type CreateClient = unsafe extern "C" fn(Handle, *const c_char) -> Handle;
        let create: CreateClient =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_create_client")?) };
        let destroy: Destroy = unsafe { std::mem::transmute(self.library.symbol(c"mpv_destroy")?) };
        let handle = unsafe { create(self.handle, c"enigmax_popup".as_ptr()) };
        if handle.is_null() {
            return Err("No se pudo crear el canal de entrada del menú".into());
        }
        Ok(PopupClient {
            handle,
            destroy,
            parent: self,
        })
    }
    #[cfg(test)]
    pub fn screenshot_pixels(&self) -> Result<(usize, usize, Vec<u8>), String> {
        self.screenshot_pixels_kind("subtitles")
    }
    #[cfg(test)]
    pub fn screenshot_pixels_kind(&self, kind: &str) -> Result<(usize, usize, Vec<u8>), String> {
        let kind = c(kind)?;
        type Execute = unsafe extern "C" fn(Handle, *const *const c_char, *mut Node) -> c_int;
        type Release = unsafe extern "C" fn(*mut Node);
        let call: Execute =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_command_ret")?) };
        let release: Release =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_free_node_contents")?) };
        let args = [
            c"screenshot-raw".as_ptr(),
            kind.as_ptr(),
            c"rgba".as_ptr(),
            ptr::null(),
        ];
        let mut output = Node {
            value: NodeValue { number: 0 },
            format: 0,
        };
        let status = unsafe { call(self.handle, args.as_ptr(), &mut output) };
        let result = (|| {
            self.check(status)?;
            if output.format != 8 {
                return Err("Captura sin mapa".into());
            }
            // mpv owns and validates the node graph until free_node_contents below.
            let list = unsafe { output.value.list.as_ref() }.ok_or("Captura vacía")?;
            if !(1..=16).contains(&list.count) || list.keys.is_null() || list.values.is_null() {
                return Err("Mapa de captura inválido".into());
            }
            let mut w = 0;
            let mut h = 0;
            let mut stride = 0;
            let mut data = None;
            for i in 0..list.count as usize {
                let key = unsafe { CStr::from_ptr(*list.keys.add(i)) }.to_bytes();
                let value = unsafe { &*list.values.add(i) };
                if value.format == 4 {
                    let n = unsafe { value.value.number };
                    match key {
                        b"w" => w = n,
                        b"h" => h = n,
                        b"stride" => stride = n,
                        _ => {}
                    }
                } else if key == b"data" && value.format == 9 {
                    data = unsafe { value.value.bytes.as_ref() };
                }
            }
            let data = data.ok_or("Faltan píxeles")?;
            if !(1..=8192).contains(&w)
                || !(1..=8192).contains(&h)
                || stride < w * 4
                || stride > 131072
                || data.data.is_null()
                || data.size != (stride * h) as usize
                || data.size > 256 * 1024 * 1024
            {
                return Err("Tamaño de captura inválido".into());
            }
            let raw = unsafe { std::slice::from_raw_parts(data.data, data.size) };
            let pixels = raw
                .chunks_exact(stride as usize)
                .flat_map(|row| row[..w as usize * 4].iter().copied())
                .collect();
            Ok((w as usize, h as usize, pixels))
        })();
        unsafe { release(&mut output) };
        result
    }
    pub fn new(path: &Path) -> Result<Self, String> {
        let library = windows::Library::load(path)?;
        // SAFETY: names and signatures match the libmpv client.h ABI; library stays alive.
        unsafe {
            let create: Create = std::mem::transmute(library.symbol(c"mpv_create")?);
            let destroy: Destroy = std::mem::transmute(library.symbol(c"mpv_terminate_destroy")?);
            let init: Init = std::mem::transmute(library.symbol(c"mpv_initialize")?);
            let option: Set = std::mem::transmute(library.symbol(c"mpv_set_option_string")?);
            let set: Set = std::mem::transmute(library.symbol(c"mpv_set_property_string")?);
            let command: Command = std::mem::transmute(library.symbol(c"mpv_command")?);
            let command_async: CommandAsync =
                std::mem::transmute(library.symbol(c"mpv_command_async")?);
            let get: Get = std::mem::transmute(library.symbol(c"mpv_get_property_string")?);
            let free: Free = std::mem::transmute(library.symbol(c"mpv_free")?);
            let wait: Wait = std::mem::transmute(library.symbol(c"mpv_wait_event")?);
            let error: Error = std::mem::transmute(library.symbol(c"mpv_error_string")?);
            let name: Name = std::mem::transmute(library.symbol(c"mpv_client_name")?);
            let version: Version = std::mem::transmute(library.symbol(c"mpv_client_api_version")?);
            if version() >> 16 != 2 {
                return Err("Versión de API libmpv incompatible: se requiere 2.x".into());
            }
            let handle = create();
            if handle.is_null() {
                return Err("No se pudo crear libmpv".into());
            }
            Ok(Self {
                handle,
                library,
                destroy,
                init,
                option,
                set,
                command,
                command_async,
                seeks: RefCell::default(),
                get,
                free,
                wait,
                error,
                name,
                registry: None,
            })
        }
    }
    fn check(&self, code: c_int) -> Result<(), String> {
        if code >= 0 {
            Ok(())
        } else {
            // SAFETY: mpv returns a static NUL-terminated error message.
            Err(unsafe { CStr::from_ptr((self.error)(code)) }
                .to_string_lossy()
                .into_owned())
        }
    }
    pub fn option(&self, key: &str, value: &str) -> Result<(), String> {
        let (k, v) = (c(key)?, c(value)?);
        self.check(unsafe { (self.option)(self.handle, k.as_ptr(), v.as_ptr()) })
            .map_err(|e| format!("Opción {key}: {e}"))
    }
    pub fn init(&self) -> Result<(), String> {
        self.check(unsafe { (self.init)(self.handle) })
    }
    pub fn observe_string(&self, key: &str, id: u64) -> Result<(), String> {
        type Observe = unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int;
        let f: Observe =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_observe_property")?) };
        let key = c(key)?;
        self.check(unsafe { f(self.handle, id, key.as_ptr(), 1) })
    }
    pub fn register_mvc(&mut self, registry: Box<crate::mvc::Registry>) -> Result<(), String> {
        type Register = unsafe extern "C" fn(
            Handle,
            *const c_char,
            *mut c_void,
            crate::mvc::OpenCallback,
        ) -> c_int;
        let register: Register =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_stream_cb_add_ro")?) };
        self.check(unsafe {
            register(
                self.handle,
                c"cine-mvc".as_ptr(),
                (&*registry as *const crate::mvc::Registry)
                    .cast_mut()
                    .cast(),
                crate::mvc::open,
            )
        })?;
        self.check(unsafe {
            register(
                self.handle,
                c"cine-disc".as_ptr(),
                (&*registry as *const crate::mvc::Registry)
                    .cast_mut()
                    .cast(),
                crate::mvc::open,
            )
        })?;
        self.registry = Some(registry);
        Ok(())
    }
    pub fn mvc(&self) -> Result<&crate::mvc::Registry, String> {
        self.registry
            .as_deref()
            .ok_or_else(|| "Protocolo MVC no registrado".into())
    }
    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        let (k, v) = (c(key)?, c(value)?);
        self.check(unsafe { (self.set)(self.handle, k.as_ptr(), v.as_ptr()) })
            .map_err(|e| format!("{key}: {e}"))
    }
    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let strings = args.iter().map(|s| c(s)).collect::<Result<Vec<_>, _>>()?;
        let mut pointers = strings.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        pointers.push(ptr::null());
        self.check(unsafe { (self.command)(self.handle, pointers.as_ptr()) })
            .map_err(|e| format!("Comando {}: {e}", args.first().copied().unwrap_or("vacío")))
    }
    pub fn get(&self, key: &str) -> Option<String> {
        let key = c(key).ok()?;
        let p = unsafe { (self.get)(self.handle, key.as_ptr()) };
        if p.is_null() {
            return None;
        }
        let result = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { (self.free)(p.cast()) };
        Some(result)
    }
    pub fn command_async(&self, args: &[&str], cookie: u64) -> Result<(), String> {
        let strings = args.iter().map(|s| c(s)).collect::<Result<Vec<_>, _>>()?;
        let mut pointers = strings.iter().map(|s| s.as_ptr()).collect::<Vec<_>>();
        pointers.push(ptr::null());
        self.check(unsafe { (self.command_async)(self.handle, cookie, pointers.as_ptr()) })
    }
    pub fn reset_seeks(&self) {
        self.seeks.borrow_mut().clear();
    }
    fn can_seek(&self) -> bool {
        self.get("idle-active").as_deref() == Some("no")
            && self.get("seekable").as_deref() == Some("yes")
    }
    fn submit_seek(&self, id: u64, request: cineplayer_core::seek::Request) -> Result<(), String> {
        if !self.can_seek() {
            self.reset_seeks();
            return Ok(());
        }
        let mut seconds = request.seconds;
        if request.absolute {
            let end = self
                .get("duration")
                .and_then(|s| s.parse::<f64>().ok())
                .filter(|v| v.is_finite() && *v > 0.0);
            seconds = seconds.max(0.0);
            if let Some(end) = end {
                seconds = seconds.min((end - 0.05).max(0.0));
            }
        }
        if let Err(e) = self.command_async(
            &[
                "seek",
                &seconds.to_string(),
                if request.absolute {
                    "absolute+exact"
                } else {
                    "relative+exact"
                },
            ],
            id,
        ) {
            self.reset_seeks();
            return Err(format!("No se pudo solicitar la búsqueda: {e}"));
        }
        Ok(())
    }
    pub fn seek(&self, seconds: f64, absolute: bool) -> Result<(), String> {
        if !seconds.is_finite() {
            return Err("Posición de búsqueda no válida".into());
        }
        if !self.can_seek() {
            return Ok(());
        }
        let next = self
            .seeks
            .borrow_mut()
            .push(cineplayer_core::seek::Request { seconds, absolute });
        if let Some((id, request)) = next {
            self.submit_seek(id, request)?;
        }
        Ok(())
    }
    pub fn seek_reply(&self, id: u64) -> Result<(), String> {
        let next = self.seeks.borrow_mut().complete(id);
        if let Some((id, request)) = next {
            self.submit_seek(id, request)?;
        }
        Ok(())
    }
    pub fn command_named(&self, name: &str, args: &[(&str, String)]) -> Result<(), String> {
        type Execute = unsafe extern "C" fn(Handle, *const Node, *mut Node) -> c_int;
        type Release = unsafe extern "C" fn(*mut Node);
        let call: Execute =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_command_node")?) };
        let release: Release =
            unsafe { std::mem::transmute(self.library.symbol(c"mpv_free_node_contents")?) };
        let mut pairs = vec![(c("name")?, c(name)?)];
        for (k, v) in args {
            pairs.push((c(k)?, c(v)?));
        }
        let mut keys = pairs
            .iter()
            .map(|(k, _)| k.as_ptr().cast_mut())
            .collect::<Vec<_>>();
        let mut values = pairs
            .iter()
            .map(|(_, v)| Node {
                value: NodeValue {
                    string: v.as_ptr().cast_mut(),
                },
                format: 1,
            })
            .collect::<Vec<_>>();
        let mut list = NodeList {
            count: values.len() as i32,
            values: values.as_mut_ptr(),
            keys: keys.as_mut_ptr(),
        };
        let input = Node {
            value: NodeValue { list: &mut list },
            format: 8,
        };
        let mut output = Node {
            value: NodeValue { number: 0 },
            format: 0,
        };
        // client.h: storage above remains live for this synchronous call; only
        // the result is allocated by mpv and must be released with its API.
        let result = unsafe { call(self.handle, &input, &mut output) };
        unsafe { release(&mut output) };
        self.check(result)
            .map_err(|e| format!("Comando {name}: {e}"))
    }
    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr((self.name)(self.handle)) }
            .to_string_lossy()
            .into_owned()
    }
    /// user-data's string conversion is JSON, including quotes around strings.
    pub fn user_data(&self, key: &str) -> Option<String> {
        let raw = self.get(key)?;
        Some(serde_json::from_str::<String>(&raw).unwrap_or(raw))
    }
    /// The event is borrowed from libmpv and valid only until the next wait call.
    pub fn wait(&mut self, timeout: f64) -> &Event {
        unsafe { &*(self.wait)(self.handle, timeout) }
    }
}

pub struct PopupClient<'a> {
    handle: Handle,
    destroy: Destroy,
    parent: &'a Mpv,
}
impl PopupClient<'_> {
    pub fn name(&self) -> String {
        unsafe { CStr::from_ptr((self.parent.name)(self.handle)) }
            .to_string_lossy()
            .into_owned()
    }
    pub fn next(&mut self) -> Result<Option<Vec<String>>, String> {
        let event = unsafe { &*(self.parent.wait)(self.handle, 0.25) };
        if event.id == 1 {
            return Err("El reproductor se está cerrando".into());
        }
        if event.id != 16 || event.data.is_null() {
            return Ok(None);
        }
        let data = unsafe { &*event.data.cast::<ClientMessage>() };
        if !(1..=8).contains(&data.count) || data.args.is_null() {
            return Ok(None);
        }
        let mut result = Vec::new();
        for i in 0..data.count as usize {
            let value = unsafe { *data.args.add(i) };
            if value.is_null() {
                return Ok(None);
            }
            let text = unsafe { CStr::from_ptr(value) }.to_string_lossy();
            if text.len() > 2048 {
                return Ok(None);
            }
            result.push(text.into_owned());
        }
        Ok(Some(result))
    }
}
impl Drop for PopupClient<'_> {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.handle) };
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // All client resources are released before unloading the DLL.
        unsafe { (self.destroy)(self.handle) };
        let _ = &self.library;
    }
}
