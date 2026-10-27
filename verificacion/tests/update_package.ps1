param([string]$Root=([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))))
$ErrorActionPreference='Stop'
. (Join-Path $Root 'src/update_package.ps1')
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
$base=Join-Path ([IO.Path]::GetTempPath()) ('Enigmax-package-tests-'+[Guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($base)
$passed=0
function Assert($Condition,[string]$Message) {if (!$Condition) {throw $Message}}
function Reject([scriptblock]$Action,[string]$Name) {
    $rejected=$false; try {& $Action} catch {$rejected=$true}
    Assert $rejected "No se rechazó: $Name"
    $script:passed++; Write-Output "OK rechazo: $Name"
}
function Write-Zip([string]$Path,$Entries) {
    $zip=[IO.Compression.ZipFile]::Open($Path,[IO.Compression.ZipArchiveMode]::Create)
    try {foreach ($name in $Entries) {
        $entry=$zip.CreateEntry($name); $stream=$entry.Open()
        try {$stream.WriteByte(1)} finally {$stream.Dispose()}
    }} finally {$zip.Dispose()}
}
try {
    foreach ($name in @('../EnigmaxCinePlayer.exe','runtime/../libmpv-2.dll','C:/evil.dll','config/default.ini','runtime/a.dll:stream','runtime\a.dll','assets/evil.lua','movie.mkv','runtime/.dll')) {
        Assert (!(Test-PayloadPath $name)) "Ruta aceptada: $name"; $passed++
    }
    foreach ($name in @('runtime/libstdc++-6.dll','runtime/libgobject-2.0-0.dll')) {
        Assert (Test-PayloadPath $name) "Dependencia DLL rechazada: $name"; $passed++
    }
    foreach ($name in @('runtime/CON.dll','runtime/com1.dll','runtime/NUL.dll')) {
        Assert (!(Test-PayloadPath $name)) "Nombre de dispositivo aceptado: $name"; $passed++
    }
    foreach ($names in @(@('../EnigmaxCinePlayer.exe'),@('libmpv-2.dll','LIBMPV-2.DLL'),@('config/default.ini'))) {
        $path=Join-Path $base ([Guid]::NewGuid().ToString('N')+'.zip'); Write-Zip $path $names
        $stage=Join-Path $base ('.enigmax-update-'+[Guid]::NewGuid().ToString('N'))
        Reject {Expand-ValidatedPayload $path $stage '1.0.0'} 'ZIP con rutas fuera del paquete o duplicadas'
    }
    $source=Join-Path $base 'source'; [void][IO.Directory]::CreateDirectory($source)
    foreach ($dir in @('runtime','assets')) {[void][IO.Directory]::CreateDirectory((Join-Path $source $dir))}
    Copy-Item -LiteralPath (Join-Path $Root 'EnigmaxCinePlayer.exe') -Destination $source
    foreach ($name in @('libmpv-2.dll','runtime/cine_mvc.dll','assets/enigmax_controls.lua')) {[IO.File]::WriteAllText((Join-Path $source $name),'synthetic test fixture')}
    $version=[Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $source 'EnigmaxCinePlayer.exe')).ProductVersion
    $files=@(foreach ($f in @('EnigmaxCinePlayer.exe','libmpv-2.dll','runtime/cine_mvc.dll','assets/enigmax_controls.lua')) {
        $p=Join-Path $source $f; [pscustomobject]@{path=$f;bytes=(Get-Item -LiteralPath $p).Length;sha256=Get-Sha256 $p}
    })
    $payload=[pscustomobject]@{format=1;version=$version;files=$files}
    $many=@($files)+@(1..252 | ForEach-Object {[pscustomobject]@{path="runtime/test-$_.dll";bytes=1;sha256=('0'*64)}})
    $limit=Join-Path $base 'limit.json'
    [IO.File]::WriteAllText($limit,([pscustomobject]@{format=1;version=$version;files=$many} | ConvertTo-Json -Depth 5))
    Assert (@((Read-PayloadManifest $limit $version).files).Count -eq 256) 'No se acepta el límite de 256 archivos'; $passed++
    [IO.File]::WriteAllText($limit,([pscustomobject]@{format=1;version=$version;files=@($many)+@([pscustomobject]@{path='runtime/overflow.dll';bytes=1;sha256=('0'*64)})} | ConvertTo-Json -Depth 5))
    Reject {Read-PayloadManifest $limit $version} 'más de 256 archivos'
    [IO.File]::WriteAllText((Join-Path $source 'package.manifest.json'),($payload | ConvertTo-Json -Depth 5))
    $zipPath=Join-Path $base 'valid.zip'
    $zip=[IO.Compression.ZipFile]::Open($zipPath,[IO.Compression.ZipArchiveMode]::Create)
    try {foreach ($f in @($files | ForEach-Object {$_.path})+@('package.manifest.json')) {
        $null=[IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip,(Join-Path $source $f),$f)
    }} finally {$zip.Dispose()}
    $stage=Join-Path $base ('.enigmax-update-'+[Guid]::NewGuid().ToString('N'))
    $actual=Expand-ValidatedPayload $zipPath $stage $version; $passed++
    $dest=Join-Path $base 'dest'; [void][IO.Directory]::CreateDirectory((Join-Path $dest 'config'))
    [IO.File]::WriteAllText((Join-Path $dest 'config/default.ini'),'user settings')
    [IO.File]::WriteAllText((Join-Path $dest 'movie.mkv'),'user original')
    Install-PortablePayload $stage $dest $actual
    Assert-PayloadFiles $dest $actual
    Assert ([IO.File]::ReadAllText((Join-Path $dest 'config/default.ini')) -ceq 'user settings') 'Ajustes alterados'
    Assert ([IO.File]::ReadAllText((Join-Path $dest 'movie.mkv')) -ceq 'user original') 'Original alterado'; $passed++
    $stage2=Join-Path $base ('.enigmax-update-'+[Guid]::NewGuid().ToString('N'))
    $actual=Expand-ValidatedPayload $zipPath $stage2 $version
    $before=Get-Sha256 (Join-Path $dest 'EnigmaxCinePlayer.exe')
    $lock=[IO.File]::Open((Join-Path $dest 'runtime/cine_mvc.dll'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    try {Reject {Install-PortablePayload $stage2 $dest $actual} 'archivo de destino bloqueado con recuperación'} finally {$lock.Dispose()}
    Assert ((Get-Sha256 (Join-Path $dest 'EnigmaxCinePlayer.exe')) -ceq $before) 'No se restauró el EXE'
    Assert-PayloadFiles $dest $actual; $passed++
    # Remove a program-owned DLL from an earlier manifest, but never config.
    $obsolete=Join-Path $dest 'runtime/obsolete-test.dll'
    [IO.File]::WriteAllText($obsolete,'obsolete synthetic DLL')
    $old=[pscustomobject]@{format=1;version=$version;files=@($actual.files)+@([pscustomobject]@{path='runtime/obsolete-test.dll';bytes=(Get-Item $obsolete).Length;sha256=Get-Sha256 $obsolete})}
    [IO.File]::WriteAllText((Join-Path $dest 'package.manifest.json'),($old | ConvertTo-Json -Depth 5))
    $stage3=Join-Path $base ('.enigmax-update-'+[Guid]::NewGuid().ToString('N'))
    $new=Expand-ValidatedPayload $zipPath $stage3 $version
    Install-PortablePayload $stage3 $dest $new
    Assert (!(Test-Path -LiteralPath $obsolete)) 'Se conservó una DLL obsoleta del manifiesto'
    Assert ([IO.File]::ReadAllText((Join-Path $dest 'config/default.ini')) -ceq 'user settings') 'Ajustes alterados al retirar DLL'; $passed++
    [IO.File]::AppendAllText((Join-Path $stage2 'libmpv-2.dll'),'altered')
    Reject {Assert-PayloadFiles $stage2 $actual} 'archivo alterado'
    Reject {Read-PayloadManifest (Join-Path $source 'package.manifest.json') '999.0.0'} 'versión incorrecta'
    # NTFS directory junctions are possible without developer-mode symlinks.
    $link=Join-Path $base 'junction'
    $null=New-Item -ItemType Junction -Path $link -Target $dest
    Reject {Get-PayloadTarget $link 'libmpv-2.dll'} 'unión NTFS en el destino'
    [IO.Directory]::Delete($link)
    Reject {Remove-OwnedStage $dest $base} 'limpieza fuera de carpeta temporal propia'
    Write-Output "$passed comprobaciones de paquete y recuperación correctas (Windows PowerShell $($PSVersionTable.PSVersion))."
} finally {
    # This exact GUID folder was created above, never a media or program directory.
    $full=[IO.Path]::GetFullPath($base)
    if ([IO.Path]::GetDirectoryName($full) -eq [IO.Path]::GetTempPath().TrimEnd('\') -and
        [IO.Path]::GetFileName($full) -match '^Enigmax-package-tests-[a-f0-9]{32}$') {
        Remove-Item -LiteralPath $full -Recurse -Force
    } else {throw 'Ruta de pruebas inesperada; no se borra'}
}
