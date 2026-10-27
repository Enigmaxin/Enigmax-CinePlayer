param([string]$ToolsPath = (Join-Path $PSScriptRoot '..\..\work\tools'))
$ErrorActionPreference = 'Stop'
$tools = [IO.Path]::GetFullPath($ToolsPath)
$build = [IO.Path]::GetFullPath((Join-Path $tools '..\build'))
if (Test-Path -LiteralPath (Join-Path $tools 'cargo\bin\cargo.exe')) {
    $env:CARGO_HOME = Join-Path $tools 'cargo'
    $env:RUSTUP_HOME = Join-Path $tools 'rustup'
    $env:PATH = "$tools\cargo\bin;$tools\w64devkit\bin;$env:PATH"
}
$env:CARGO_TARGET_DIR = $build
# GCC 16 in this w64devkit places the unwinder in libgcc.a. Rust's GNU
# target still requests libgcc_eh.a. Supply the same verified archive by that name.
if (Test-Path -LiteralPath (Join-Path $tools 'w64devkit\bin\gcc.exe')) {
    $gcc = Join-Path $tools 'w64devkit\bin\gcc.exe'
    $archive = (& $gcc -print-libgcc-file-name).Trim()
    $compat = Join-Path $build 'link-compat'
    New-Item -ItemType Directory -Force -Path $compat | Out-Null
    Copy-Item -LiteralPath $archive -Destination (Join-Path $compat 'libgcc_eh.a')
    $env:CARGO_ENCODED_RUSTFLAGS = "-Lnative=$compat"
}
& (Join-Path $PSScriptRoot 'build-native.ps1') -ToolsPath $tools
$manifest = Join-Path $PSScriptRoot 'Cargo.toml'
& cargo fmt --manifest-path $manifest -- --check
if ($LASTEXITCODE -ne 0) { throw 'Formato Rust incorrecto' }
& cargo clippy --locked --manifest-path $manifest --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw 'Ha fallado el análisis estático' }
& cargo test --locked --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw 'Han fallado las pruebas' }
& cargo build --release --locked --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw 'Ha fallado la compilación' }
Copy-Item -LiteralPath (Join-Path $build 'release\EnigmaxCinePlayer.exe') -Destination (Join-Path $PSScriptRoot 'EnigmaxCinePlayer.exe')
Write-Output 'Compilación completada. Conserva libmpv-2.dll junto a EnigmaxCinePlayer.exe.'
