# Package validation and recoverable replacement. No network or UI in this module.
# Paths in a release are data: never pass them to a shell or overwrite config/media.
function Get-Sha256([string]$Path) {
    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-','').ToLowerInvariant() }
    finally { $sha.Dispose(); $stream.Dispose() }
}

function Test-PayloadPath([string]$Path) {
    if ($Path -ceq 'EnigmaxCinePlayer.exe' -or $Path -ceq 'libmpv-2.dll' -or
        $Path -ceq 'assets/enigmax_controls.lua' -or $Path -ceq 'package.manifest.json') { return $true }
    if ($Path -cmatch '^runtime/[a-zA-Z0-9_-]+\.dll$' -or
        $Path -cmatch '^licencias/[a-zA-Z0-9_.-]+\.(txt|md)$' -or
        $Path -cin @('LICENSE.txt','README.md','DEPENDENCIAS.md','INSTALACION.txt','ACTUALIZACIONES.md')) { return $true }
    return $false
}

function Assert-NoReparse([string]$Path) {
    $cursor = [IO.Path]::GetFullPath($Path)
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "No se actualizan rutas con enlaces o uniones: $cursor" }
        }
        $parent = [IO.Path]::GetDirectoryName($cursor)
        if ($parent -eq $cursor) { break }; $cursor = $parent
    }
}

function Get-PayloadTarget([string]$Root, [string]$Relative) {
    if (!(Test-PayloadPath $Relative)) { throw "Archivo no permitido: $Relative" }
    $base = [IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
    $target = [IO.Path]::GetFullPath((Join-Path $base $Relative))
    if (!$target.StartsWith($base,[StringComparison]::OrdinalIgnoreCase)) { throw 'Ruta fuera del paquete' }
    Assert-NoReparse $target
    if (Test-Path -LiteralPath $target -PathType Container) { throw "Se esperaba un archivo: $target" }
    return $target
}

function Read-PayloadManifest([string]$Path, [string]$ExpectedVersion) {
    if ((Get-Item -LiteralPath $Path).Length -gt 131072) { throw 'Manifiesto demasiado grande' }
    $m = [IO.File]::ReadAllText($Path,[Text.Encoding]::UTF8) | ConvertFrom-Json
    if ($m.format -ne 1 -or $m.version -cnotmatch '^\d+\.\d+\.\d+$' -or
        ($ExpectedVersion -and $m.version -cne $ExpectedVersion) -or
        @($m.files).Count -lt 4 -or @($m.files).Count -gt 128) { throw 'Manifiesto de paquete no válido' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [long]$total = 0
    foreach ($f in $m.files) {
        if (!(Test-PayloadPath ([string]$f.path)) -or $f.path -ieq 'package.manifest.json' -or
            !$seen.Add([string]$f.path) -or $f.sha256 -cnotmatch '^[a-f0-9]{64}$' -or
            $f.bytes -isnot [ValueType] -or [double]$f.bytes -ne [long]$f.bytes -or
            $f.bytes -lt 1 -or $f.bytes -gt 536870912) { throw 'Entrada de manifiesto no válida' }
        $total += [long]$f.bytes
        if ($total -gt 1073741824) { throw 'Paquete expandido demasiado grande' }
    }
    foreach ($required in @('EnigmaxCinePlayer.exe','libmpv-2.dll','runtime/cine_mvc.dll','assets/enigmax_controls.lua')) {
        if (!$seen.Contains($required)) { throw "Falta $required" }
    }
    return $m
}

function Assert-PayloadFiles([string]$Root, $Payload) {
    foreach ($f in $Payload.files) {
        $p = Get-PayloadTarget $Root $f.path
        if (!(Test-Path -LiteralPath $p -PathType Leaf) -or (Get-Item -LiteralPath $p).Length -ne $f.bytes -or
            (Get-Sha256 $p) -cne $f.sha256) { throw "Archivo ausente o alterado: $($f.path)" }
    }
    $exe = Get-PayloadTarget $Root 'EnigmaxCinePlayer.exe'
    $pe = [IO.BinaryReader]::new([IO.File]::OpenRead($exe))
    try {
        if ($pe.ReadUInt16() -ne 0x5A4D) { throw 'El reproductor no es PE' }
        $pe.BaseStream.Position = 0x3C; $offset = $pe.ReadUInt32()
        if ($offset -gt $pe.BaseStream.Length-6) { throw 'Cabecera PE no válida' }
        $pe.BaseStream.Position = $offset
        if ($pe.ReadUInt32() -ne 0x4550 -or $pe.ReadUInt16() -ne 0x8664) { throw 'El reproductor no es Windows x64' }
    } finally { $pe.Dispose() }
    $version = [Diagnostics.FileVersionInfo]::GetVersionInfo($exe).ProductVersion
    if ($version -cne $Payload.version -and $version -cne "$($Payload.version).0") { throw 'Versión PE distinta a la del paquete' }
}

function Expand-ValidatedPayload([string]$Zip, [string]$Stage, [string]$Version) {
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    Assert-NoReparse $Stage
    if (Test-Path -LiteralPath $Stage) { throw 'La carpeta de preparación ya existe' }
    [void][IO.Directory]::CreateDirectory($Stage)
    $archive = [IO.Compression.ZipFile]::OpenRead($Zip)
    try {
        if ($archive.Entries.Count -gt 129) { throw 'Demasiadas entradas ZIP' }
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        [long]$total = 0
        # Validate ALL names and bounds before extracting anything. No directory,
        # link, alternate stream, traversal, case alias, or arbitrary file entries.
        foreach ($entry in $archive.Entries) {
            if (!(Test-PayloadPath $entry.FullName) -or !$seen.Add($entry.FullName) -or
                $entry.Length -lt 1 -or $entry.Length -gt 536870912 -or
                (($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000) { throw 'Entrada ZIP no permitida' }
            $total += $entry.Length
            if ($total -gt 1073872896) { throw 'ZIP expandido demasiado grande' }
        }
        if (!$seen.Contains('package.manifest.json')) { throw 'Falta el manifiesto del ZIP' }
        foreach ($entry in $archive.Entries) {
            $target = Get-PayloadTarget $Stage $entry.FullName
            [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target))
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$target,$false)
        }
    } finally { $archive.Dispose() }
    $payload = Read-PayloadManifest (Join-Path $Stage 'package.manifest.json') $Version
    if ($seen.Count -ne @($payload.files).Count+1) { throw 'El ZIP contiene archivos fuera del manifiesto' }
    foreach ($f in $payload.files) { if (!$seen.Contains($f.path)) { throw 'El ZIP no coincide con el manifiesto' } }
    Assert-PayloadFiles $Stage $payload
    return $payload
}

function Install-PortablePayload([string]$Stage, [string]$Destination, $Payload) {
    Assert-PayloadFiles $Stage $Payload
    $paths = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($f in $Payload.files) { [void]$paths.Add($f.path) }
    [void]$paths.Add('package.manifest.json')
    $previous = Get-PayloadTarget $Destination 'package.manifest.json'
    if (Test-Path -LiteralPath $previous) {
        $old = Read-PayloadManifest $previous ''
        foreach ($f in $old.files) { [void]$paths.Add($f.path) }
    }
    # All affected targets are checked before the first move. A rollback stays
    # on the same volume and contains only exact program-owned files.
    foreach ($p in $paths) { $null = Get-PayloadTarget $Destination $p }
    $backup = Join-Path $Stage 'rollback'
    [void][IO.Directory]::CreateDirectory($backup)
    $moved = [Collections.Generic.List[string]]::new()
    $placed = [Collections.Generic.List[string]]::new()
    try {
        foreach ($p in $paths) {
            $target = Get-PayloadTarget $Destination $p
            if (Test-Path -LiteralPath $target -PathType Leaf) {
                $save = Get-PayloadTarget $backup $p
                [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($save))
                [IO.File]::Move($target,$save); $moved.Add($p)
            }
        }
        foreach ($p in @($Payload.files | ForEach-Object {$_.path}) + @('package.manifest.json')) {
            $target = Get-PayloadTarget $Destination $p
            [void][IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target))
            [IO.File]::Move((Get-PayloadTarget $Stage $p),$target); $placed.Add($p)
        }
        Assert-PayloadFiles $Destination $Payload
    } catch {
        $original = $_.Exception.Message
        try {
            foreach ($p in $placed) { [IO.File]::Delete((Get-PayloadTarget $Destination $p)) }
            foreach ($p in $moved) { [IO.File]::Move((Get-PayloadTarget $backup $p),(Get-PayloadTarget $Destination $p)) }
        } catch {
            throw "RECUPERACION PENDIENTE: conserva $Stage. Error: $original. Recuperación: $($_.Exception.Message)"
        }
        throw "Se ha restaurado la versión anterior. $original"
    }
}

function Remove-OwnedStage([string]$Stage, [string]$Destination) {
    $full = [IO.Path]::GetFullPath($Stage)
    if ([IO.Path]::GetDirectoryName($full) -cne [IO.Path]::GetFullPath($Destination).TrimEnd('\') -or
        [IO.Path]::GetFileName($full) -cnotmatch '^\.enigmax-update-[a-f0-9]{32}$') { throw 'Limpieza fuera de la carpeta de actualización' }
    Assert-NoReparse $full
    if (Test-Path -LiteralPath $full) {
        foreach ($item in Get-ChildItem -LiteralPath $full -Force -Recurse) {
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Enlace inesperado en preparación; se conserva la carpeta' }
        }
        Remove-Item -LiteralPath $full -Recurse -Force
    }
}
