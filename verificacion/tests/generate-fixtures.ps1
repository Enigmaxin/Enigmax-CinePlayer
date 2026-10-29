param(
    [Parameter(Mandatory=$true)][string]$Ffmpeg,
    [string]$Destination=([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../muestras')))
)
# Entirely synthetic media: generated frames, sine waves and test text.
# Existing files are never overwritten. No original movie is read or copied.
$ErrorActionPreference='Stop'
$ff=(Get-Item -LiteralPath $Ffmpeg).FullName
$dest=[IO.Path]::GetFullPath($Destination)
[void][IO.Directory]::CreateDirectory($dest)
function Write-NewText([string]$Name,[string]$Text) {
    $p=Join-Path $dest $Name
    if(!(Test-Path -LiteralPath $p)) {[IO.File]::WriteAllText($p,$Text,[Text.UTF8Encoding]::new($false))}
    return $p
}
$sourceSbs=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../muestras/ass3d-sbs.ass'))
$sbs=Write-NewText 'ass3d-sbs.ass' ([IO.File]::ReadAllText($sourceSbs))
$ou=Write-NewText 'ass3d-ou.ass' ([IO.File]::ReadAllText($sbs).Replace('PlayResX: 3840','PlayResX: 1920').Replace('PlayResY: 1080','PlayResY: 2160').Replace('pos(2872,940)','pos(952,2020)'))
$srt=Write-NewText 'synthetic.srt' "1`n00:00:00,000 --> 00:00:29,000`nSubtitulos de prueba`n"
foreach($case in @(
    @('ass3d-black.mkv','1920x1080',''),
    @('ass3d-hsbs.mkv','1920x1080',$sbs),
    @('ass3d-hou.mkv','1920x1080',$ou),
    @('ass3d-fullsbs.mkv','3840x1080',$sbs),
    @('ass3d-fullou.mkv','1920x2160',$ou),
    @('ass3d-tagged-fullsbs.mkv','3840x1080',$sbs),
    @('ass3d-tagged-fullou.mkv','1920x2160',$ou)
)) {
    $path=Join-Path $dest $case[0]
    if(Test-Path -LiteralPath $path){continue}
    $args=@('-hide_banner','-loglevel','error','-nostdin','-n','-f','lavfi','-i',("color=black:s="+$case[1]+':r=24:d=6'))
    if($case[2]) {$args+=@('-i',$case[2],'-i',$srt,'-map','0:v','-map','1:0','-map','2:0','-c:s:0','ass','-c:s:1','srt')}
    $args+=@('-c:v','libx264','-preset','ultrafast','-threads','2','-pix_fmt','yuv420p','-t','6')
    if($case[0] -like '*tagged-fullsbs*'){$args+=@('-metadata:s:v:0','stereo_mode=left_right')}
    if($case[0] -like '*tagged-fullou*'){$args+=@('-metadata:s:v:0','stereo_mode=top_bottom')}
    & $ff @args $path
    if($LASTEXITCODE -ne 0){throw "No se pudo generar $path"}
}
$full=Join-Path $dest 'fullsbs.mkv'
if(!(Test-Path -LiteralPath $full)) {
    # Different eye colours are essential: a black frame cannot detect a stale shader.
    & $ff -hide_banner -loglevel error -nostdin -n -f lavfi -i 'color=green:s=3840x1080:r=24:d=6,drawbox=x=1920:y=0:w=1920:h=1080:color=red:t=fill,drawbox=x=760:y=340:w=400:h=400:color=white:t=fill' -an -c:v libx264 -preset ultrafast -threads 2 -pix_fmt yuv420p $full
    if($LASTEXITCODE -ne 0){throw 'No se pudo generar fullsbs.mkv'}
}
$tracks=Join-Path $dest 'pistas-qa.mkv'
if(!(Test-Path -LiteralPath $tracks)) {
    & $ff -hide_banner -loglevel error -nostdin -n -f lavfi -i 'color=black:s=1280x720:r=24:d=30' -f lavfi -i 'sine=frequency=440:duration=30' -f lavfi -i 'sine=frequency=880:duration=30' -i $srt -i $sbs -map 0:v -map 1:a -map 2:a -map 3:0 -map 4:0 -c:v libx264 -preset ultrafast -threads 2 -pix_fmt yuv420p -c:a aac -c:s:0 srt -c:s:1 ass -t 30 $tracks
    if($LASTEXITCODE -ne 0){throw 'No se pudo generar pistas-qa.mkv'}
}
Write-Output "Muestras sinteticas preparadas: $dest"

$disc=Join-Path $dest 'disc-clear.m2ts'
if(!(Test-Path -LiteralPath $disc)) {
    & $ff -hide_banner -loglevel error -nostdin -n -f lavfi -i 'testsrc2=s=320x180:r=24:d=6' -f lavfi -i 'sine=frequency=440:duration=6' -c:v libx264 -threads 2 -preset ultrafast -pix_fmt yuv420p -c:a aac -f mpegts $disc
    if($LASTEXITCODE -ne 0){throw 'No se pudo generar el flujo de disco sintético'}
}

$chapters=Join-Path $dest 'chapters-qa.mkv'
if(!(Test-Path -LiteralPath $chapters)) {
    $meta=Write-NewText 'chapters-qa.ffmeta' ";FFMETADATA1`n[CHAPTER]`nTIMEBASE=1/1000`nSTART=0`nEND=8000`ntitle=Inicio`n[CHAPTER]`nTIMEBASE=1/1000`nSTART=8000`nEND=16000`ntitle=Segundo capítulo`n[CHAPTER]`nTIMEBASE=1/1000`nSTART=16000`nEND=30000`ntitle=Final`n"
    & $ff -hide_banner -loglevel error -nostdin -n -i $tracks -i $meta -map 0 -map_metadata 1 -map_chapters 1 -c copy $chapters
    if($LASTEXITCODE -ne 0){throw 'No se pudo generar la muestra con capítulos'}
}
