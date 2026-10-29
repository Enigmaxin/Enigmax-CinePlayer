//! One short-lived worker per application session. No network work on the UI thread.
use cineplayer_core::{APP_VERSION, update_policy};
use std::{
    ffi::c_void,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const HELPER: &str = concat!(
    include_str!("updater.ps1"),
    "\n",
    include_str!("update_package.ps1"),
    "\n",
    include_str!("update_apply.ps1")
);
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(hwnd: *mut c_void, text: *const u16, title: *const u16, flags: u32) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateMutexW(attributes: *mut c_void, owner: i32, name: *const u16) -> *mut c_void;
    fn GetLastError() -> u32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn dialog(text: &str, question: bool) -> bool {
    show(text, if question { 0x124 } else { 0x10 })
}
fn show(text: &str, flags: u32) -> bool {
    let text = wide(text);
    let title = wide("Enigmax CinePlayer — Actualizaciones");
    // Yes/No, default No. This worker owns its dialog; playback keeps running.
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), flags) == 6 }
}
fn powershell(script: &Path) -> Result<Command, String> {
    let system = std::env::var_os("SystemRoot").ok_or("Falta SystemRoot")?;
    let exe = PathBuf::from(system).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut cmd = Command::new(exe);
    cmd.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "RemoteSigned",
        "-File",
    ])
    .arg(script)
    .creation_flags(CREATE_NO_WINDOW)
    .stdin(Stdio::null());
    Ok(cmd)
}
fn save_cache(
    dir: &Path,
    path: &Path,
    cache: &cineplayer_core::update_cache::Cache,
) -> Result<(), String> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    file.write_all(&cache.encode()).map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    crate::windows::replace_file(file.path(), path)
}
fn check(manual: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let config = exe
        .parent()
        .ok_or("Falta la carpeta del programa")?
        .join("config");
    std::fs::create_dir_all(&config).map_err(|e| e.to_string())?;
    let cache_path = config.join("updater-cache.json");
    let mut cache = cache_path
        .metadata()
        .ok()
        .filter(|m| m.len() <= 8192)
        .and_then(|_| std::fs::read(&cache_path).ok())
        .map(|b| cineplayer_core::update_cache::Cache::decode(&b))
        .unwrap_or_default();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let wait = cache.wait(now, manual);
    if wait > 0 {
        if manual {
            show(
                &format!(
                    "La comprobación está en espera. Próximo intento dentro de {} minutos.\nÚltimo estado: {}\n\nDescargas: https://github.com/Enigmaxin/Enigmax-CinePlayer/releases",
                    wait.div_ceil(60),
                    match cache.status.as_str() {
                        "ok" => "Comprobación correcta",
                        "no-release" => "No hay una versión con manifiesto publicado",
                        "limited" => "Límite temporal de peticiones",
                        "http-error" => "Error del servidor",
                        other => other,
                    }
                ),
                0x40,
            );
        }
        return Ok(());
    }
    // Record before connecting too, so crashes/offline attempts cannot flood GitHub.
    cache.record(now, "Comprobación iniciada o conexión interrumpida", 0);
    save_cache(&config, &cache_path, &cache)?;
    let temp = tempfile::Builder::new()
        .prefix("Enigmax-CinePlayer-update-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let script = temp.path().join("updater.ps1");
    // Windows PowerShell 5.1 requires a BOM to read Spanish text reliably.
    std::fs::write(
        &script,
        format!("\u{feff}{}", HELPER.replace('\u{feff}', "")),
    )
    .map_err(|e| e.to_string())?;
    let result = powershell(&script)?
        .arg("-Mode")
        .arg("Query")
        .arg("-Endpoint")
        .arg(update_policy::UPDATE_ENDPOINT)
        .arg("-AppVersion")
        .arg(APP_VERSION)
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).trim().to_string());
    }
    let root: serde_json::Value = serde_json::from_slice(&result.stdout)
        .map_err(|e| format!("Respuesta de actualización no válida: {e}"))?;
    let status = root["status"].as_str().unwrap_or("invalid");
    let retry = root["retry"].as_u64().unwrap_or(0);
    if status != "ok" {
        let message = match status {
            "no-release" => "No hay una versión publicada con manifiesto de actualización.",
            "limited" => {
                "GitHub ha limitado las peticiones. Se respetará el plazo de espera indicado por el servidor."
            }
            _ => "El servidor no pudo completar la comprobación de actualizaciones.",
        };
        cache.record(now, status, retry);
        save_cache(&config, &cache_path, &cache)?;
        let info = format!(
            "{message}\nHTTP: {}. Próximo intento dentro de {} minutos.\nDescargas: https://github.com/Enigmaxin/Enigmax-CinePlayer/releases",
            root["http"],
            cache.wait(now, true).div_ceil(60)
        );
        std::fs::write(config.join("updater-status.log"), &info).map_err(|e| e.to_string())?;
        if manual {
            show(&info, 0x40);
        }
        return Ok(());
    }
    let json = serde_json::to_vec(&root["release"]).map_err(|e| e.to_string())?;
    let available = update_policy::manifest(&json, APP_VERSION)?;
    cache.record(now, "ok", 0);
    save_cache(&config, &cache_path, &cache)?;
    std::fs::write(
        config.join("updater-status.log"),
        "Comprobación completada correctamente.",
    )
    .map_err(|e| e.to_string())?;
    let Some(release) = available else {
        if manual {
            show(
                &format!(
                    "No hay una versión estable más reciente.\nVersión instalada: {APP_VERSION}"
                ),
                0x40,
            );
        }
        return Ok(());
    };
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if !dialog(
        &format!(
            "Hay una nueva versión: {} (actual: {}).\n\nSe descargarán {:.1} MB. Tras verificar el ZIP portátil, se cerrará el reproductor, se actualizarán el EXE y sus bibliotecas en esta misma carpeta conservando los ajustes y se reiniciará. Cierra las otras instancias antes de actualizar.\n\n¿Quieres actualizar ahora?",
            release.version,
            APP_VERSION,
            release.size as f64 / 1_000_000.0
        ),
        true,
    ) {
        return Ok(());
    }
    let manifest = serde_json::json!({
        "url":release.url,"size":release.size,"sha256":release.sha256,
        "version":release.version.to_string(),"exe":exe,
        "pid":std::process::id()
    });
    let manifest_path = temp.path().join("update.json");
    std::fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let mut helper = powershell(&script)?
        .arg("-Mode")
        .arg("Apply")
        .arg("-Manifest")
        .arg(&manifest_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let ready = temp.path().join("ready");
    let deadline = Instant::now() + Duration::from_secs(3600);
    loop {
        if ready.is_file() {
            // The helper has verified the download and holds a handle to this
            // process. It exclusively owns the temporary directory from now on.
            let _ = temp.keep();
            std::process::exit(0);
        }
        if let Some(status) = helper.try_wait().map_err(|e| e.to_string())? {
            if status.success() {
                return Ok(());
            }
            if status.code() != Some(2) {
                dialog(
                    "No se pudo iniciar o completar el auxiliar de actualización. El reproductor sigue abierto. Comprueba los permisos y las políticas de PowerShell.",
                    false,
                );
            }
            return Err(
                "El auxiliar no pudo completar la actualización; el reproductor sigue abierto."
                    .into(),
            );
        }
        if Instant::now() > deadline {
            let _ = helper.kill();
            let _ = helper.wait();
            return Err("Tiempo de actualización agotado".into());
        }
        std::thread::sleep(Duration::from_millis(150));
    }
}
pub fn start() {
    launch(false);
}
pub fn start_manual() {
    launch(true);
}
fn launch(manual: bool) {
    let _ = std::thread::Builder::new()
        .name("github-updater".into())
        .spawn(move || {
            let name = wide("Local\\EnigmaxCinePlayer.Updater");
            let mutex = unsafe { CreateMutexW(std::ptr::null_mut(), 0, name.as_ptr()) };
            if mutex.is_null() {
                if manual {dialog("No se pudo iniciar la comprobación de actualizaciones.",false);}
                return;
            }
            let existed = unsafe { GetLastError() } == 183;
            if existed && manual {show("Ya hay una comprobación o actualización en curso. Vuelve a intentarlo cuando termine.",0x40);}
            if !existed && let Err(error) = check(manual) {
                // Offline/rate-limited/missing-release errors are non-intrusive.
                if let Ok(exe) = std::env::current_exe()
                    && let Some(dir) = exe.parent()
                {
                    let _ = std::fs::create_dir_all(dir.join("config"));
                    let _ = std::fs::write(dir.join("config/updater-status.log"), &error);
                }
                if manual {dialog(&format!("No se pudo completar la comprobación: {error}\n\nSe reintentará automáticamente más tarde.\nDescargas: https://github.com/Enigmaxin/Enigmax-CinePlayer/releases"),false);}
            }
            unsafe {
                CloseHandle(mutex);
            }
        });
}
