param(
    [string]$ToolsPath = (Join-Path $PSScriptRoot '..\..\work\tools'),
    [string]$FfmpegDirectory = (Join-Path $PSScriptRoot 'native\vendor\ffmpeg'),
    [string]$RuntimeDirectory = (Join-Path $PSScriptRoot 'runtime')
)
$ErrorActionPreference='Stop'
$tools=[IO.Path]::GetFullPath($ToolsPath)
$env:PATH="$tools\w64devkit\bin;$env:PATH"
$nativeTests=[IO.Path]::GetFullPath((Join-Path $tools '..\build'))
New-Item -ItemType Directory -Force $nativeTests | Out-Null
$annexbTest=Join-Path $nativeTests 'test_annexb.exe'
& gcc -std=c11 -O2 -Wall -Wextra -Werror (Join-Path $PSScriptRoot 'native/test_annexb.c') -o $annexbTest
if($LASTEXITCODE -ne 0){throw 'Error al compilar la prueba Annex B'}
& $annexbTest
if($LASTEXITCODE -ne 0){throw 'Error de reconstrucción Annex B'}
$ff=[IO.Path]::GetFullPath($FfmpegDirectory)
$edge=Join-Path $PSScriptRoot 'native\vendor\edge264-mvc'
& make -C $edge OS=windows STATIC=yes BUILDTEST=no VARIANTS=x86-64-v2,x86-64-v3 'CFLAGS=-march=x86-64 -mtune=generic' -j4
if($LASTEXITCODE -ne 0){throw 'Error al compilar edge264-mvc'}
$udf=Join-Path $PSScriptRoot 'native\vendor\libudfread\src'
foreach($p in @("$ff\include\libavformat\avformat.h","$edge\libedge264mvc.a","$udf\udfread.c")){if(!(Test-Path -LiteralPath $p)){throw "Falta una dependencia nativa: $p"}}
$runtime=[IO.Path]::GetFullPath($RuntimeDirectory)
New-Item -ItemType Directory -Force $runtime | Out-Null
$c=Join-Path $PSScriptRoot 'native\cine_mvc.c'
$args=@('-std=gnu11','-O2','-march=x86-64','-mtune=generic','-shared','-static-libgcc','-Wall','-Wextra','-Werror=implicit-function-declaration','-Wno-misleading-indentation','-DHAVE_FCNTL_H=1','-I',"$ff\include",'-I',$edge,'-I',$udf,$c)
foreach($name in @('default_blockinput','ecma167','udf_volume','udf_fs','udfread')){$args+=Join-Path $udf "$name.c"}
$args+=@((Join-Path $edge 'libedge264mvc.a'),'-L',"$ff\lib",'-lavformat','-lavcodec','-lavutil','-pthread','-o',(Join-Path $runtime 'cine_mvc.dll'))
& gcc @args
if($LASTEXITCODE -ne 0){throw 'Error de compilación de la biblioteca MVC'}

Write-Output 'Biblioteca MVC/UDF compilada; dependencia cargada bajo demanda.'
