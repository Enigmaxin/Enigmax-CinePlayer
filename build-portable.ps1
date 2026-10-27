param([string]$OutputDirectory=(Split-Path $PSScriptRoot -Parent))
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
. (Join-Path $PSScriptRoot 'src/update_package.ps1')
$version=[regex]::Match([IO.File]::ReadAllText((Join-Path $PSScriptRoot 'Cargo.toml')),'(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"').Groups[1].Value
if (!$version) {throw 'No se pudo leer la versión'}
$output=[IO.Path]::GetFullPath($OutputDirectory)
[void][IO.Directory]::CreateDirectory($output)
$package=Join-Path $output "Enigmax-CinePlayer-Portable-$version-windows-x64.zip"
if (Test-Path -LiteralPath $package) {throw "El paquete ya existe: $package. Retíralo explícitamente antes de volver a empaquetar."}
$files=@('EnigmaxCinePlayer.exe','libmpv-2.dll','assets/enigmax_controls.lua','LICENSE.txt','README.md','DEPENDENCIAS.md','INSTALACION.txt','ACTUALIZACIONES.md')
$files+=Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'runtime') -File -Filter '*.dll' | ForEach-Object {'runtime/'+$_.Name}
$files+=Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot 'licencias') -File | ForEach-Object {'licencias/'+$_.Name}
$hashes=@(foreach ($f in ($files | Sort-Object -Unique)) {
    $p=Get-PayloadTarget $PSScriptRoot $f
    [pscustomobject]@{path=$f;bytes=(Get-Item -LiteralPath $p).Length;sha256=Get-Sha256 $p}
})
$manifest=Join-Path $PSScriptRoot 'package.manifest.json'
$payload=[pscustomobject]@{format=1;version=$version;files=$hashes}
Assert-PayloadFiles $PSScriptRoot $payload
[IO.File]::WriteAllText($manifest,($payload | ConvertTo-Json -Depth 5),[Text.UTF8Encoding]::new($false))
$partial=$package+'.partial'
if (Test-Path -LiteralPath $partial) {throw "Paquete parcial pendiente: $partial"}
$zip=[IO.Compression.ZipFile]::Open($partial,[IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($f in @($hashes | ForEach-Object {$_.path})+@('package.manifest.json')) {
        $null=[IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip,(Join-Path $PSScriptRoot $f),$f,[IO.Compression.CompressionLevel]::Optimal)
    }
} finally {$zip.Dispose()}
# Verify every file again from the resulting archive, before it becomes current.
$stage=Join-Path $output ('.enigmax-update-'+[Guid]::NewGuid().ToString('N'))
try {
    $null=Expand-ValidatedPayload $partial $stage $version
    [IO.File]::Move($partial,$package)
} finally {Remove-OwnedStage $stage $output}
$sum=Get-Sha256 $package
[IO.File]::WriteAllText($package+'.sha256',"$sum  $([IO.Path]::GetFileName($package))`n",[Text.UTF8Encoding]::new($false))
Write-Output "ZIP portátil verificado: $package"
