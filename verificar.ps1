param([string]$Destination = (Join-Path $env:TEMP ('EnigmaxCinePlayer-QA-' + [guid]::NewGuid().ToString('N'))))
$ErrorActionPreference='Stop'
$dest=[IO.Path]::GetFullPath($Destination)
if(Test-Path -LiteralPath $dest){throw 'El destino debe ser una carpeta nueva para conservar las verificaciones previas.'}
New-Item -ItemType Directory -Path $dest | Out-Null
$cases=@(
 @('fullsbs.mkv','fsbs'),@('hsbs.mkv','hsbs'),@('hou.mkv','hou'),@('fullsbs.mkv','fsbs-right'),
 @('uhd_hdr10.mkv','original'),@('uhd_hdr10.mkv','original-sdr'),@('srt_audio.mkv','original'),@('ass_audio.mkv','original'),
 @('fullsbs.mkv','fsbs-ana'),@('hsbs.mkv','hsbs-ana'),@('hou.mkv','hou-ana'),@('fullou.mkv','fou-ana'),
 @('fullsbs.mkv','fsbs-ana-gm'),@('fullsbs.mkv','fsbs-ana-ab'),@('fullsbs.mkv','fsbs-ana-rb'),@('fullsbs.mkv','fsbs-ana-rg'),
 @('fullsbs.mkv','fsbs-ana-cpu'),@('fullsbs.mkv','fsbs-ana-ab-half-cpu'),@('fullsbs.mp4','fsbs-ana'),
 @('srt_audio.mkv','hsbs-ana'),@('ass_audio.mkv','hsbs-ana'),@('uhd_hdr10.mkv','hsbs-ana')
)
foreach($case in $cases){
 $sample=Join-Path $PSScriptRoot "verificacion/muestras/$($case[0])"
 $name="$($case[0])-$($case[1])"
 $capture=Join-Path $dest "$name.png"
 $p=Start-Process -FilePath (Join-Path $PSScriptRoot 'EnigmaxCinePlayer.exe') -ArgumentList @('--probe',('"'+$sample+'"'),$case[1],('"'+$capture+'"')) -WindowStyle Hidden -PassThru
 if(!$p.WaitForExit(40000)){throw "Tiempo agotado: $name"}
 if($p.ExitCode -ne 0 -or !(Test-Path -LiteralPath $capture)){throw "Fallo: $name"}
 $report=Get-Content -LiteralPath ([IO.Path]::ChangeExtension($capture,'.txt')) -Raw -Encoding utf8
 if($case[1] -in @('fsbs','hsbs','hou','fsbs-right') -and ($report -notmatch 'video-out-params/dw: 1920' -or $report -notmatch 'video-out-params/dh: 1080')){throw "Geometría incorrecta: $name"}
 if($case[1] -match 'ana' -and $report -notmatch 'video-aspect-override: 1.777778'){throw "Aspecto incorrecto: $name"}
 if($case[1] -match 'ana' -and $case[1] -notmatch 'cpu' -and $report -notmatch 'Enigmax anaglyph linear RGB'){throw "Shader sin ejecutar: $name"}
 if($case[1] -match 'cpu' -and $report -notmatch 'stereo3d='){throw "Filtro CPU sin aplicar: $name"}
 if($case[0] -eq 'srt_audio.mkv' -and $report -notmatch 'sub-text: Prueba SRT'){throw "SRT no renderizado: $name"}
 if($case[0] -eq 'ass_audio.mkv' -and $report -notmatch 'sub-text: Prueba ASS'){throw "ASS no renderizado: $name"}
 if($case[1] -match 'sdr|ana' -and $report -notmatch 'target-trc: bt.1886'){throw "Destino SDR incorrecto: $name"}
 Write-Output "$name : OK"
}
Write-Output "22 casos correctos. Informes y capturas sintéticas: $dest"
Write-Output 'No certifica TV HDR, passthrough físico, PGS, otras máquinas ni sesiones largas.'

