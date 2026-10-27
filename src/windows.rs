use std::{
    cell::RefCell,
    ffi::{CStr, c_void},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    ptr,
    time::{Duration, Instant},
};
type Handle = *mut c_void;
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(path: *const u16, file: Handle, flags: u32) -> Handle;
    fn GetProcAddress(module: Handle, name: *const i8) -> *mut c_void;
    fn FreeLibrary(module: Handle) -> i32;
    fn SetLastError(error: u32);
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    fn AddDllDirectory(path: *const u16) -> Handle;
    fn RemoveDllDirectory(cookie: Handle) -> i32;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(owner: Handle, text: *const u16, title: *const u16, flags: u32) -> i32;
    fn CreatePopupMenu() -> Handle;
    fn AppendMenuW(menu: Handle, flags: u32, id: usize, text: *const u16) -> i32;
    fn DestroyMenu(menu: Handle) -> i32;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn TrackPopupMenu(
        menu: Handle,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        owner: Handle,
        rect: *const c_void,
    ) -> i32;
    fn SetForegroundWindow(window: Handle) -> i32;
    fn GetForegroundWindow() -> Handle;
    fn GetWindowThreadProcessId(window: Handle, process: *mut u32) -> u32;
    fn PostMessageW(window: Handle, message: u32, w: usize, l: isize) -> i32;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: Handle,
    ) -> Handle;
    fn DestroyWindow(window: Handle) -> i32;
    fn ShowWindow(window: Handle, command: i32) -> i32;
    fn IsWindow(window: Handle) -> i32;
    fn SetPropW(window: Handle, name: *const u16, value: Handle) -> i32;
    fn GetPropW(window: Handle, name: *const u16) -> Handle;
    fn RemovePropW(window: Handle, name: *const u16) -> Handle;
    fn PeekMessageW(message: *mut Message, window: Handle, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn PostQuitMessage(code: i32);
    fn MsgWaitForMultipleObjectsEx(
        count: u32,
        handles: *const Handle,
        timeout: u32,
        wake: u32,
        flags: u32,
    ) -> u32;
    fn LoadImageW(
        instance: Handle,
        name: *const u16,
        kind: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Handle;
    fn DestroyIcon(icon: Handle) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn SendMessageTimeoutW(
        window: Handle,
        message: u32,
        w: usize,
        l: isize,
        flags: u32,
        timeout: u32,
        result: *mut usize,
    ) -> isize;
}
#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
#[derive(Default)]
struct Message {
    window: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}

/// An unowned window keeps the controller and mpv VO input queues independent.
/// Reuse it so the WM_NULL posted after tracking is actually consumed alive.
struct MenuHost(Handle);
const MENU_MARKER: &str = "EnigmaxCinePlayer.PopupHost";
impl MenuHost {
    fn valid(&self) -> bool {
        unsafe {
            IsWindow(self.0) != 0
                && GetPropW(self.0, wide(MENU_MARKER).as_ptr()) == GetModuleHandleW(ptr::null())
        }
    }
    fn new() -> Result<Self, String> {
        let h = unsafe {
            CreateWindowExW(
                0x80,
                wide("STATIC").as_ptr(),
                wide("Enigmax CinePlayer — Menús").as_ptr(),
                0x80000000,
                0,
                0,
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        if h.is_null() {
            return Err(format!(
                "No se pudo crear la ventana del menú: {}",
                std::io::Error::last_os_error()
            ));
        }
        if unsafe { SetPropW(h, wide(MENU_MARKER).as_ptr(), GetModuleHandleW(ptr::null())) } == 0 {
            unsafe {
                DestroyWindow(h);
            }
            return Err("No se pudo identificar la ventana del menú".into());
        }
        Ok(Self(h))
    }
    fn pump(&self) {
        let mut message = Message::default();
        for _ in 0..64 {
            if unsafe { PeekMessageW(&mut message, self.0, 0, 0, 1) } == 0 {
                break;
            }
            if message.message == 0x12 {
                unsafe {
                    PostQuitMessage(message.wparam as i32);
                }
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}
impl Drop for MenuHost {
    fn drop(&mut self) {
        if self.valid() {
            unsafe {
                RemovePropW(self.0, wide(MENU_MARKER).as_ptr());
                DestroyWindow(self.0);
            }
        }
    }
}
thread_local! {static MENU_HOST: RefCell<Option<MenuHost>> = const {RefCell::new(None)};}

struct DependencyDirectory(Handle);
impl Drop for DependencyDirectory {
    fn drop(&mut self) {
        unsafe { RemoveDllDirectory(self.0) };
    }
}
pub struct Library {
    handle: Handle,
    // Keep the registered, application-owned runtime directory alive while
    // the module and its delayed dependencies may need it. Never add PATH/CWD.
    _runtime: Option<DependencyDirectory>,
}
pub fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    let a = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let b = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 1 | 8) } == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
impl Library {
    pub fn load(path: &Path) -> Result<Self, String> {
        let resolved =
            std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let runtime = resolved.parent().map(|p| p.join("runtime"));
        let runtime = if let Some(path) = runtime.filter(|p| p.is_dir()) {
            let path = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            let wide = path
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>();
            let cookie = unsafe { AddDllDirectory(wide.as_ptr()) };
            if cookie.is_null() {
                return Err(format!(
                    "No se pudo registrar runtime: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Some(DependencyDirectory(cookie))
        } else {
            None
        };
        let path = resolved
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // DLL folder, application directory, explicitly registered runtime and
        // system locations. LOAD_LIBRARY_SEARCH_DEFAULT_DIRS excludes PATH/CWD.
        let handle = unsafe { LoadLibraryExW(path.as_ptr(), ptr::null_mut(), 0x100 | 0x1000) };
        if handle.is_null() {
            Err(format!(
                "No se pudo cargar {}: {}",
                resolved.display(),
                std::io::Error::last_os_error()
            ))
        } else {
            Ok(Self {
                handle,
                _runtime: runtime,
            })
        }
    }
    pub fn symbol(&self, name: &CStr) -> Result<*mut c_void, String> {
        let p = unsafe { GetProcAddress(self.handle, name.as_ptr()) };
        if p.is_null() {
            Err(format!("libmpv no exporta {}", name.to_string_lossy()))
        } else {
            Ok(p)
        }
    }
}
impl Drop for Library {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.handle) };
    }
}

pub fn message(text: &str, error: bool) {
    unsafe {
        MessageBoxW(
            ptr::null_mut(),
            wide(text).as_ptr(),
            wide("Enigmax CinePlayer").as_ptr(),
            if error { 0x10 } else { 0x40 },
        )
    };
}

/// Owned icon resources. Keep this alive until after libmpv destroys its window.
pub struct Branding {
    small: Handle,
    large: Handle,
    applied_to: usize,
}

impl Branding {
    pub fn new() -> Result<Self, String> {
        // Resource ordinal 1 is embedded by assets/windows.rc.
        let module = unsafe { GetModuleHandleW(ptr::null()) };
        // MAKEINTRESOURCEW(1): this ordinal token is never dereferenced by Rust.
        let resource = std::ptr::without_provenance::<u16>(1);
        let small = unsafe {
            LoadImageW(
                module,
                resource,
                1,
                GetSystemMetrics(49),
                GetSystemMetrics(50),
                0,
            )
        };
        let large = unsafe {
            LoadImageW(
                module,
                resource,
                1,
                GetSystemMetrics(11),
                GetSystemMetrics(12),
                0,
            )
        };
        if small.is_null() || large.is_null() {
            let error = std::io::Error::last_os_error();
            if !small.is_null() {
                unsafe { DestroyIcon(small) };
            }
            if !large.is_null() {
                unsafe { DestroyIcon(large) };
            }
            return Err(format!(
                "No se pudo cargar el icono de Enigmax CinePlayer: {error}"
            ));
        }
        Ok(Self {
            small,
            large,
            applied_to: 0,
        })
    }

    pub fn apply(&mut self, window: usize) -> Result<(), String> {
        if window == 0 || self.applied_to == window {
            return Ok(());
        }
        let mut result = 0;
        for (size, icon) in [(0, self.small), (1, self.large)] {
            // WM_SETICON. A timeout prevents a stalled video thread blocking the controller.
            if unsafe {
                SendMessageTimeoutW(
                    window as Handle,
                    0x80,
                    size,
                    icon as isize,
                    2,
                    1000,
                    &mut result,
                )
            } == 0
            {
                return Err(format!(
                    "No se pudo aplicar el icono: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        self.applied_to = window;
        Ok(())
    }
}

impl Drop for Branding {
    fn drop(&mut self) {
        unsafe {
            DestroyIcon(self.small);
            DestroyIcon(self.large);
        }
    }
}

/// Native menu; Windows provides mouse/keyboard navigation, accessibility and DPI scaling.
pub struct Menu(Handle);
impl Menu {
    pub fn new() -> Result<Self, String> {
        let h = unsafe { CreatePopupMenu() };
        if h.is_null() {
            Err("No se pudo crear el menú de Windows".into())
        } else {
            Ok(Self(h))
        }
    }
    pub fn item(&self, id: u32, text: &str, checked: bool) -> Result<(), String> {
        // Ampersands in media/track/device names must not become keyboard mnemonics.
        let text = wide(&text.replace('&', "&&"));
        let ok = unsafe {
            AppendMenuW(
                self.0,
                if checked { 8 } else { 0 },
                id as usize,
                text.as_ptr(),
            )
        };
        if ok == 0 {
            Err("No se pudo añadir una opción al menú".into())
        } else {
            Ok(())
        }
    }
    /// Read-only status line; Windows excludes it from keyboard navigation.
    pub fn label(&self, text: &str) -> Result<(), String> {
        let text = wide(&text.replace('&', "&&"));
        if unsafe { AppendMenuW(self.0, 1, 0, text.as_ptr()) } == 0 {
            Err("No se pudo mostrar el estado del menú".into())
        } else {
            Ok(())
        }
    }
    pub fn submenu(&self, text: &str, child: Menu) -> Result<(), String> {
        let ok = unsafe { AppendMenuW(self.0, 0x10, child.0 as usize, wide(text).as_ptr()) };
        if ok == 0 {
            return Err("No se pudo añadir el submenú".into());
        }
        std::mem::forget(child); // DestroyMenu(parent) also destroys its submenus.
        Ok(())
    }
    pub fn separator(&self) {
        unsafe { AppendMenuW(self.0, 0x800, 0, ptr::null()) };
    }
    pub fn show(&self, m: &crate::mpv::Mpv) -> Result<u32, String> {
        let window = m.get("window-id").and_then(|v| v.parse().ok()).unwrap_or(0);
        // Pause only overlay input/timing, not the movie. The Lua script also
        // keeps the bar visible and resets its activity time on return.
        let _ = m.command(&["script-message-to", "enigmax_controls", "popup", "yes"]);
        let result = self.show_window(window);
        let _ = m.command(&["script-message-to", "enigmax_controls", "popup", "no"]);
        result
    }
    fn show_window(&self, window: usize) -> Result<u32, String> {
        let foreground = unsafe { GetForegroundWindow() };
        let mut pid = 0;
        if !foreground.is_null() {
            unsafe {
                GetWindowThreadProcessId(foreground, &mut pid);
            }
            if pid != std::process::id() {
                return Ok(0);
            } // stale request: do not steal another app's focus
        }
        let owner = if window == 0 {
            foreground
        } else {
            window as Handle
        };
        if owner.is_null() || unsafe { IsWindow(owner) } == 0 {
            return Ok(0);
        }
        unsafe {
            GetWindowThreadProcessId(owner, &mut pid);
        }
        if pid != std::process::id() {
            return Err("La ventana del reproductor no es válida".into());
        }
        let mut point = Point::default();
        if unsafe { GetCursorPos(&mut point) } == 0 {
            return Err("No se pudo localizar el cursor".into());
        }
        MENU_HOST.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.as_ref().is_none_or(|h| !h.valid()) {
                *slot = Some(MenuHost::new()?);
            }
            let host = slot.as_ref().ok_or("Falta la ventana del menú")?;
            host.pump();
            unsafe {
                ShowWindow(host.0, 4);
                SetForegroundWindow(host.0);
            }
            let until = Instant::now() + Duration::from_millis(200);
            loop {
                host.pump();
                if unsafe { GetForegroundWindow() } == host.0 {
                    break;
                }
                if Instant::now() >= until {
                    unsafe {
                        ShowWindow(host.0, 0);
                    }
                    return Err("Windows no pudo activar el menú del reproductor".into());
                }
                unsafe {
                    MsgWaitForMultipleObjectsEx(0, ptr::null(), 10, 0x04FF, 0x4);
                }
            }
            unsafe {
                SetLastError(0);
            }
            let id = unsafe {
                TrackPopupMenu(
                    self.0,
                    0x100 | 0x2,
                    point.x,
                    point.y,
                    0,
                    host.0,
                    ptr::null(),
                )
            };
            let error = std::io::Error::last_os_error();
            unsafe {
                PostMessageW(host.0, 0, 0, 0);
            }
            host.pump(); // WM_NULL must reach a live window, not one just destroyed
            let current = unsafe { GetForegroundWindow() };
            let restore = current == host.0 || current == owner;
            unsafe {
                ShowWindow(host.0, 0);
            }
            if restore && unsafe { IsWindow(owner) } != 0 {
                unsafe {
                    SetForegroundWindow(owner);
                }
                let mut result = 0;
                // Activation across queues is asynchronous. Wait boundedly for
                // the video window to process it before accepting another menu.
                unsafe {
                    SendMessageTimeoutW(owner, 0, 0, 0, 2, 200, &mut result);
                }
            }
            if id == 0 && error.raw_os_error().unwrap_or(0) != 0 {
                Err(format!("Error del menú: {error}"))
            } else {
                Ok(id as u32)
            }
        })
    }
}
impl Drop for Menu {
    fn drop(&mut self) {
        unsafe { DestroyMenu(self.0) };
    }
}

#[repr(C)]
struct OpenFileName {
    size: u32,
    owner: Handle,
    instance: Handle,
    filter: *const u16,
    custom: *mut u16,
    max_custom: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    title_file: *mut u16,
    max_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    extension: u16,
    default_ext: *const u16,
    custom_data: isize,
    hook: *mut c_void,
    template: *const u16,
    reserved: *mut c_void,
    reserved2: u32,
    flags_ex: u32,
}
#[link(name = "comdlg32")]
unsafe extern "system" {
    fn GetOpenFileNameW(data: *mut OpenFileName) -> i32;
    fn CommDlgExtendedError() -> u32;
}
pub fn open_file(owner: usize, subtitles: bool) -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    let filter = wide(if subtitles {
        "Subtítulos (SRT, ASS, SSA, SUP)\0*.srt;*.ass;*.ssa;*.sup\0Todos los archivos\0*.*\0"
    } else {
        "Vídeos MKV / MP4 / Blu-ray ISO\0*.mkv;*.mp4;*.iso;*.m2ts\0Vídeos\0*.mkv;*.mp4;*.iso;*.m2ts;*.avi;*.mov;*.webm\0Todos los archivos\0*.*\0"
    });
    let title = wide(if subtitles {
        "Añadir subtítulos"
    } else {
        "Abrir vídeo"
    });
    // SAFETY: OPENFILENAMEW permits zero for all unused pointer/optional fields.
    let mut data: OpenFileName = unsafe { std::mem::zeroed() };
    data.size = std::mem::size_of::<OpenFileName>() as u32;
    data.owner = owner as Handle;
    data.filter = filter.as_ptr();
    data.filter_index = 1;
    data.file = buffer.as_mut_ptr();
    data.max_file = buffer.len() as u32;
    data.title = title.as_ptr();
    data.flags = 0x1000 | 0x800 | 0x8 | 0x80000;
    if unsafe { GetOpenFileNameW(&mut data) } != 0 {
        let n = buffer.iter().position(|x| *x == 0).unwrap_or(buffer.len());
        Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
            &buffer[..n],
        ))))
    } else {
        let error = unsafe { CommDlgExtendedError() };
        if error == 0 {
            Ok(None)
        } else {
            Err(format!("Error del selector de archivos: {error}"))
        }
    }
}
