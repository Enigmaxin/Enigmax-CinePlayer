$folder = Split-Path -LiteralPath $PSCommandPath
$form = $response = $inputStream = $outputStream = $parentProcess = $updateLock = $null
$stage = $destination = $null
$preserveStage = $parentClosed = $false
$exitCode = 1
try {
    $data = [IO.File]::ReadAllText($Manifest,[Text.Encoding]::UTF8) | ConvertFrom-Json
    $exe = [IO.Path]::GetFullPath([string]$data.exe)
    $destination = [IO.Path]::GetDirectoryName($exe)
    if ($exe -match '[\r\n]' -or [IO.Path]::GetFileName($exe) -cne 'EnigmaxCinePlayer.exe' -or
        !(Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'Ruta de aplicación no válida' }
    Assert-NoReparse $exe
    $version = [string]$data.version
    if ($version -cnotmatch '^\d+\.\d+\.\d+$' -or $data.sha256 -cnotmatch '^[a-f0-9]{64}$' -or
        $data.size -lt 4096 -or $data.size -gt 536870912) { throw 'Manifiesto no válido' }
    $uri = [Uri]$data.url
    $asset = "Enigmax-CinePlayer-Portable-$version-windows-x64.zip"
    if ($uri.Scheme -cne 'https' -or $uri.Host -cne 'github.com' -or $uri.Query -or $uri.Fragment -or
        $uri.AbsolutePath -cnotmatch ('^/Enigmaxin/Enigmax-CinePlayer/releases/download/[vV]?' + [regex]::Escape($version) + '/' + [regex]::Escape($asset) + '$')) { throw 'Origen de paquete no válido' }
    $parentProcess = [Diagnostics.Process]::GetProcessById([int]$data.pid)
    $null = $parentProcess.Handle
    if (![string]::Equals($parentProcess.MainModule.FileName,$exe,[StringComparison]::OrdinalIgnoreCase)) { throw 'El PID no corresponde al reproductor' }
    $lockPath = Join-Path $destination '.enigmax-update.lock'
    Assert-NoReparse $lockPath
    $updateLock = [IO.FileStream]::new($lockPath,[IO.FileMode]::OpenOrCreate,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None,4096,[IO.FileOptions]::DeleteOnClose)
    Add-Type -AssemblyName System.Windows.Forms
    [Windows.Forms.Application]::EnableVisualStyles()
    $form = New-Object Windows.Forms.Form
    $form.Text = 'Enigmax CinePlayer — Descargando actualización'
    $form.ClientSize = New-Object Drawing.Size(440,115)
    $form.StartPosition = 'CenterScreen'; $form.FormBorderStyle = 'FixedDialog'
    $form.MaximizeBox = $false; $form.MinimizeBox = $false
    $label = New-Object Windows.Forms.Label
    $label.SetBounds(15,12,410,24); $label.Text = "Descargando versión $version…"
    $bar = New-Object Windows.Forms.ProgressBar; $bar.SetBounds(15,42,410,22)
    $cancel = New-Object Windows.Forms.Button; $cancel.Text = 'Cancelar'; $cancel.SetBounds(330,77,95,26)
    $cancel.Add_Click({$form.Close()}); $form.Controls.AddRange(@($label,$bar,$cancel)); $form.Show()
    [Windows.Forms.Application]::DoEvents()
    $zip = Join-Path $folder 'package.zip'
    $response = Open-Https ([string]$data.url) "Enigmax-CinePlayer-Updater/$version"
    if ($response.ContentLength -ge 0 -and $response.ContentLength -ne $data.size) { throw 'Tamaño remoto inesperado' }
    $inputStream = $response.GetResponseStream()
    $outputStream = [IO.File]::Open($zip,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
    $buffer = New-Object byte[] 65536
    [long]$total = 0; $downloadStart = [DateTime]::UtcNow; $paint = [Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $read = $inputStream.ReadAsync($buffer,0,$buffer.Length)
        $readStart = [Diagnostics.Stopwatch]::StartNew()
        while (!$read.IsCompleted) {
            [Windows.Forms.Application]::DoEvents()
            if ($form.IsDisposed) { $inputStream.Dispose(); throw [OperationCanceledException]::new() }
            if ($readStart.Elapsed.TotalSeconds -gt 15) { $inputStream.Dispose(); throw 'Tiempo de lectura agotado' }
            Start-Sleep -Milliseconds 30
        }
        $count = $read.GetAwaiter().GetResult()
        if ($count -eq 0) { break }
        if ($form.IsDisposed) { throw [OperationCanceledException]::new() }
        $total += $count
        if ($total -gt $data.size) { throw 'El ZIP supera el tamaño declarado' }
        if (([DateTime]::UtcNow-$downloadStart).TotalMinutes -gt 45) { throw 'Descarga demasiado lenta' }
        $outputStream.Write($buffer,0,$count)
        if ($paint.ElapsedMilliseconds -ge 100) {
            $bar.Value = [Math]::Min(100,[int](100.0*$total/$data.size))
            $label.Text = 'Descargando: {0:N1} / {1:N1} MB' -f ($total/1e6),($data.size/1e6)
            [Windows.Forms.Application]::DoEvents(); $paint.Restart()
        }
    }
    $outputStream.Dispose(); $outputStream = $null
    $inputStream.Dispose(); $inputStream = $null; $response.Dispose(); $response = $null
    if ($form.IsDisposed) { throw [OperationCanceledException]::new() }
    $label.Text = 'Verificando el paquete portátil…'; [Windows.Forms.Application]::DoEvents()
    if ($total -ne $data.size -or (Get-Sha256 $zip) -cne $data.sha256) { throw 'El tamaño o SHA-256 del ZIP no coincide' }
    $stage = Join-Path $destination ('.enigmax-update-'+[Guid]::NewGuid().ToString('N'))
    $payload = Expand-ValidatedPayload $zip $stage $version
    # Validate write targets, including links, before asking the parent to exit.
    foreach ($f in $payload.files) { $null = Get-PayloadTarget $destination $f.path }
    if ($form.IsDisposed) { throw [OperationCanceledException]::new() }
    $form.Close(); $form.Dispose(); $form = $null
    [IO.File]::WriteAllText((Join-Path $folder 'ready'),'verified')
    if (!$parentProcess.WaitForExit(60000)) { throw 'El reproductor no se ha cerrado; no se ha sustituido ningún archivo' }
    $parentClosed = $true
    Install-PortablePayload $stage $destination $payload
    $restart = New-Object Diagnostics.ProcessStartInfo
    $restart.FileName = $exe; $restart.WorkingDirectory = $destination; $restart.UseShellExecute = $false
    $null = [Diagnostics.Process]::Start($restart)
    $exitCode = 0
} catch [OperationCanceledException] {
    $exitCode = 0
} catch {
    $message = $_.Exception.Message
    $preserveStage = $message.StartsWith('RECUPERACION PENDIENTE:')
    try {
        Add-Type -AssemblyName System.Windows.Forms
        $context = if ($parentClosed) {'No se pudo completar la actualización. Puedes abrir de nuevo el reproductor si la recuperación ha terminado.'} else {'No se ha aplicado la actualización. El reproductor sigue abierto.'}
        [void][Windows.Forms.MessageBox]::Show("$context`r`n`r`n$message",'Enigmax CinePlayer — Actualización',0,16)
        $exitCode = 2
    } catch {}
} finally {
    if ($outputStream) {$outputStream.Dispose()}; if ($inputStream) {$inputStream.Dispose()}
    if ($response) {$response.Dispose()}; if ($parentProcess) {$parentProcess.Dispose()}
    if ($form) {$form.Dispose()}
    if ($stage -and !$preserveStage) {
        try { Remove-OwnedStage $stage $destination } catch { [Console]::Error.WriteLine($_.Exception.Message) }
    }
    if ($updateLock) { $updateLock.Dispose() }
    foreach ($name in @('package.zip','update.json','ready','updater.ps1')) {
        Remove-Item -LiteralPath (Join-Path $folder $name) -Force -ErrorAction SilentlyContinue
    }
    Remove-Item -LiteralPath $folder -ErrorAction SilentlyContinue
}
exit $exitCode
