param([string]$Destination=(Join-Path $env:TEMP ('Enigmax-ASS3D-QA-'+[guid]::NewGuid().ToString('N'))))
$ErrorActionPreference='Stop'
$dest=[IO.Path]::GetFullPath($Destination)
if(Test-Path -LiteralPath $dest){throw 'Usa una carpeta nueva para conservar las pruebas anteriores.'}
New-Item -ItemType Directory -Path $dest | Out-Null
$cases=@(
 @('ass3d-hsbs.mkv','hsbs','SBS'),@('ass3d-hsbs.mkv','hsbs-right','SBS'),@('ass3d-hsbs.mkv','hsbs-ana-color','SBS'),
 @('ass3d-hsbs.mkv','hsbs-ana-gm-color','SBS'),@('ass3d-hsbs.mkv','hsbs-ana-ab-color','SBS'),
 @('ass3d-hsbs.mkv','hsbs-ana-color-swap','SBS'),@('ass3d-hsbs.mkv','hsbs-ana-color-cal','SBS'),
 @('ass3d-hsbs.mkv','hsbs-both','SBS'),@('ass3d-hsbs.mkv','hsbs-ana-cpu','SBS'),
 @('ass3d-hsbs.mkv','hsbs-ana-suboff','no'),@('ass3d-hou.mkv','hou','OU'),@('ass3d-hou.mkv','hou-right','OU'),
 @('ass3d-hou.mkv','hou-ana-color','OU'),@('ass3d-hou.mkv','hou-both','OU'),
 @('ass_audio.mkv','hsbs-ana','no'),@('srt_audio.mkv','hsbs-ana','no'),
 @('ass3d-hsbs.mkv','hsbs-ana-subforce','SBS'),@('ass3d-hou.mkv','hou-ana-subforce','OU'),
 @('ass3d-fullsbs.mkv','fsbs','SBS'),@('ass3d-fullsbs.mkv','fsbs-ana-color','SBS'),@('ass3d-fullsbs.mkv','fsbs-both','SBS'),
 @('ass3d-fullou.mkv','fou','OU'),@('ass3d-fullou.mkv','fou-ana-color','OU'),@('ass3d-fullou.mkv','fou-both','OU')
)
foreach($c in $cases){
 $name="$($c[0])-$($c[1])";$sample=Join-Path $PSScriptRoot "verificacion/muestras/$($c[0])";$capture=Join-Path $dest "$name.png"
 $p=Start-Process -FilePath (Join-Path $PSScriptRoot 'EnigmaxCinePlayer.exe') -ArgumentList @('--probe',('"'+$sample+'"'),$c[1],('"'+$capture+'"')) -WindowStyle Hidden -PassThru
 if(!$p.WaitForExit(40000)){throw "Tiempo agotado: $name"}
 if($p.ExitCode -ne 0 -or !(Test-Path -LiteralPath $capture)){throw "Fallo: $name"}
 $text=Get-Content -LiteralPath ([IO.Path]::ChangeExtension($capture,'.txt')) -Raw -Encoding utf8
 if($c[2] -eq 'no'){
  if($text -notmatch 'current-vo: gpu-next' -or $text -notmatch 'blend-subtitles: no'){throw "Se ha alterado un subtítulo 2D o desactivado: $name"}
 }else{
  if($text -notmatch 'current-vo: gpu\r?\n' -or $text -notmatch 'blend-subtitles: video' -or $text -notmatch "ASS 3D: ASS 3D $($c[2])"){throw "No se ha activado ASS 3D: $name"}
  if($text -notmatch 'blend subs video'){throw "Subtítulos sin fusionar antes del shader: $name"}
  if($c[1] -match 'ana' -and $text -notmatch 'user shader: Enigmax anaglyph'){throw "Sin composición conjunta anaglifa: $name"}
  if($c[1] -notmatch 'ana|both' -and $text -notmatch 'Enigmax eye selection ASS and video'){throw "Sin selección conjunta de ojo: $name"}
 }
 Write-Output "$name : OK"
}
Write-Output "24 casos ASS 3D correctos. Resultados: $dest"
Write-Output 'No sustituye la evaluación binocular con gafas ni certifica cualquier script ASS.'
