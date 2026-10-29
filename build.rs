use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/windows.rc");
    println!("cargo:rerun-if-changed=assets/windows.manifest");
    println!("cargo:rerun-if-changed=assets/EnigmaxCinePlayer.ico");
    println!("cargo:rerun-if-changed=assets/choice-atlas.png");
    println!("cargo:rerun-if-changed=native/cinema_dialog.c");
    println!("cargo:rerun-if-changed=native/cinema_menu.h");
    println!("cargo:rerun-if-env-changed=WINDRES");
    // The shared geometry library remains buildable on non-Windows hosts.
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(
        env::var("CARGO_CFG_TARGET_ENV").as_deref(),
        Ok("gnu"),
        "Esta compilación Windows necesita Rust GNU y MinGW windres"
    );
    let out =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR no disponible")).join("branding.o");
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Falta la ruta del proyecto"));
    let dialog = out.with_file_name("cinema-dialog.o");
    let status = Command::new("gcc")
        .current_dir(&root)
        .args([
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-c",
            "native/cinema_dialog.c",
            "-o",
        ])
        .arg(&dialog)
        .status()
        .expect("No se pudo compilar el selector nativo");
    assert!(status.success(), "Error al compilar el selector nativo");
    let archive = out.with_file_name("libcinema_dialog.a");
    let status = Command::new("ar")
        .arg("crs")
        .arg(&archive)
        .arg(&dialog)
        .status()
        .expect("No se pudo empaquetar el selector nativo");
    assert!(status.success(), "Error al enlazar el selector nativo");
    println!(
        "cargo:rustc-link-search=native={}",
        archive.parent().unwrap().display()
    );
    println!("cargo:rustc-link-lib=static=cinema_dialog");
    for library in [
        "gdiplus", "shlwapi", "ole32", "msimg32", "dwmapi", "gdi32", "user32", "comctl32",
    ] {
        println!("cargo:rustc-link-lib={library}");
    }
    let tool = env::var_os("WINDRES").unwrap_or_else(|| "windres".into());
    let status = Command::new(tool)
        .current_dir(&root)
        .args([
            "--input",
            "assets/windows.rc",
            "--output-format",
            "coff",
            "--output",
        ])
        .arg(&out)
        .status()
        .expect("No se pudo ejecutar windres; instala MinGW y añade su bin al PATH");
    assert!(
        status.success(),
        "No se pudieron compilar el icono y los metadatos"
    );
    println!(
        "cargo:rustc-link-arg-bin=EnigmaxCinePlayer={}",
        out.display()
    );
}
