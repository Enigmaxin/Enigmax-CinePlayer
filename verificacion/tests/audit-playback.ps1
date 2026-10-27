param(
    [string]$Root=([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))),
    [string]$PlayerPath,
    [string]$OutputDirectory
)
$ErrorActionPreference='Stop'
$version=[regex]::Match([IO.File]::ReadAllText((Join-Path $Root 'Cargo.toml')),'(?m)^version\s*=\s*"(\d+\.\d+\.\d+)"').Groups[1].Value
$out=if($OutputDirectory){[IO.Path]::GetFullPath($OutputDirectory)}else{Join-Path $Root "verificacion/$version/sync"}
if(!$PlayerPath){$PlayerPath=Join-Path $Root 'EnigmaxCinePlayer.exe'}
if(Test-Path -LiteralPath $out){throw 'Los resultados de esta auditoría ya existen'}
[void][IO.Directory]::CreateDirectory($out)
$samples=Join-Path $Root 'verificacion/muestras/sync'
$cases=@(
    @('h264-ac3-srt',(Join-Path $samples 'h264.mkv'),'original','2',0),
    @('mp4-aac',(Join-Path $samples 'fullsbs.mp4'),'fsbs-ana','',0),
    @('hsbs-eac3-ass',(Join-Path $samples 'hsbs.mkv'),'hsbs-ana','1',0),
    @('hevc10-aac',(Join-Path $samples 'hevc10.mkv'),'original','2',0),
    @('vfr-ac3',(Join-Path $samples 'vfr.mkv'),'original','2',0),
    @('mvc-talktome','F:\ISOS VO\TALKTOME_3D.iso','mvc-ana','',600),
    @('titanic-hsbs-ass','F:\Titanic 100 Years In 3D (2012) (HSBS).mkv','hsbs-ana','1',0)
)
$results=@()
foreach($case in $cases){
    if(!(Test-Path -LiteralPath $case[1])){throw "Falta la muestra autorizada: $($case[1])"}
    $capture=Join-Path $out ($case[0]+'.png')
    $env:ENIGMAX_PROBE_PLAY_SECONDS='30'; $env:ENIGMAX_PROBE_SYNC='1'
    $env:ENIGMAX_PROBE_EXERCISE='1'; $env:ENIGMAX_PROBE_AUDIO_REAL='1'
    if($case[3]){$env:ENIGMAX_PROBE_SID=$case[3]}else{Remove-Item Env:ENIGMAX_PROBE_SID -ErrorAction SilentlyContinue}
    $env:ENIGMAX_PROBE_START=[string]$case[4]
    # MP4 has no embedded subtitle; use the prepared external SRT.
    if($case[0] -eq 'mp4-aac'){$env:ENIGMAX_PROBE_SUB_FILE=Join-Path $samples 'timed.srt'}else{Remove-Item Env:ENIGMAX_PROBE_SUB_FILE -ErrorAction SilentlyContinue}
    $p=Start-Process -FilePath $PlayerPath -ArgumentList @('--probe',('"'+$case[1]+'"'),$case[2],('"'+$capture+'"')) -WindowStyle Hidden -PassThru
    $clock=[Diagnostics.Stopwatch]::StartNew(); [long]$peak=0
    while(!$p.HasExited){
        $p.Refresh(); $peak=[Math]::Max($peak,$p.PrivateMemorySize64)
        if($clock.Elapsed.TotalSeconds -gt 85){$p.Kill();throw "Prueba agotada: $($case[0])"}
        Start-Sleep -Milliseconds 150
    }
    $p.WaitForExit()
    if($p.ExitCode -ne 0){throw "Fallo de reproducción: $($case[0])"}
    $rows=@(Import-Csv ([IO.Path]::ChangeExtension($capture,'sync.csv')))
    $transitions=@{}; $stable=@(); $subtitleErrors=0; $subtitleChecks=0
    foreach($row in $rows){
        if(!$transitions.ContainsKey($row.stage)){$transitions[$row.stage]=[double]$row.elapsed}
        if([double]$row.elapsed -lt $transitions[$row.stage]+1 -or $row.seeking -eq 'yes' -or
            $row.pause -eq 'yes' -or $row.'paused-for-cache' -eq 'yes' -or !$row.avsync){continue}
        $stable+=$row
        if($case[1].StartsWith($samples) -and $row.'time-pos'){
            $t=[double]$row.'time-pos'; $phase=$t%2
            if($phase -gt 0.15 -and $phase -lt 0.85){
                $expected='SYNC_{0:000}' -f ([int][Math]::Floor($t/2)*2)
                $subtitleChecks++; if($row.'sub-text' -notlike "*$expected*"){$subtitleErrors++}
            }elseif($phase -gt 1.15 -and $phase -lt 1.85){$subtitleChecks++;if($row.'sub-text'.Trim()){$subtitleErrors++}}
        }
    }
    $drift=@($stable | ForEach-Object {[Math]::Abs([double]$_.avsync)*1000} | Sort-Object)
    $maximum=if($drift.Count){$drift[-1]}else{$null}
    $drops=($rows | ForEach-Object {[int]$_.'frame-drop-count'} | Measure-Object -Maximum).Maximum
    $result=[pscustomobject]@{case=$case[0];exit=$p.ExitCode;stableSamples=$stable.Count;maxAvMs=$maximum;
        p95AvMs=if($drift.Count){$drift[[int][Math]::Floor(($drift.Count-1)*0.95)]}else{$null};frameDrops=$drops;
        subtitleChecks=$subtitleChecks;subtitleErrors=$subtitleErrors;peakPrivateMiB=[Math]::Round($peak/1MB,1);
        pass=($stable.Count -gt 20 -and $maximum -lt 100 -and $subtitleErrors -eq 0)}
    $results+=$result; $results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $out 'results.json') -Encoding utf8
    $result | ConvertTo-Json -Compress | Write-Output
}
foreach($name in @('ENIGMAX_PROBE_PLAY_SECONDS','ENIGMAX_PROBE_SYNC','ENIGMAX_PROBE_EXERCISE','ENIGMAX_PROBE_AUDIO_REAL','ENIGMAX_PROBE_SID','ENIGMAX_PROBE_START','ENIGMAX_PROBE_SUB_FILE')){Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue}
if(@($results | Where-Object {!$_.pass}).Count){throw 'Hay casos fuera del criterio de sincronización; revisar results.json'}
Write-Output '7 casos de sincronización y estrés completados.'
