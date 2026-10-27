use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/windows.rc");
    println!("cargo:rerun-if-changed=assets/EnigmaxCinePlayer.ico");
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
