# Compilar Enigmax CinePlayer 0.2.13

El repositorio contiene el programa y puente MVC/UDF. Los dos ZIP adicionales de FUENTES.md contienen las instantáneas y dependencias del motor, sus recetas y las fuentes Cargo. No contienen películas ni ajustes personales.

## Windows x64

Rust GNU, GCC/make/windres y Windows 11. build.ps1 verifica formato, Clippy y pruebas antes de copiar el EXE. El conjunto probado usa Rust 1.99.1, w64devkit 2.10.1 y MSYS2 UCRT64/GCC 16.2.0. ToolsPath admite cargo/bin, rustup y w64devkit/bin, o herramientas ya presentes en PATH.

1. Extraer ambos ZIP de fuentes en una carpeta. fuentes/EnigmaxCinePlayer es el proyecto; fuentes/mpv y fuentes/ffmpeg son exportaciones de las revisiones fijadas. dependency-sources/ contiene las fuentes MSYS2 y los metadatos BUILDINFO/PKGINFO/PKGBUILD/SRCINFO.
2. Preparar MSYS2 UCRT64. El archivo fuentes/msys2-packages.lock registra el entorno instalado, que incluye herramientas y un superconjunto de bibliotecas. Para reconstruir una dependencia, extraer su .src.tar.zst y usar su PKGBUILD con makepkg-mingw en UCRT64, respetando las versiones/recetas registradas. Algunos paquetes originales de Rust obtienen crates durante prepare(); sus fuentes están también en fuentes/cargo-vendor. No se afirma reproducibilidad bit a bit ni se requiere usar todos los paquetes del entorno para ejecutar.
3. En bash UCRT64: bash fuentes/EnigmaxCinePlayer/native/build-engine.sh fuentes engine-build 4. El script comprueba revisiones, compila FFmpeg y luego libmpv; no reemplaza el reproductor. Si se usa Git en lugar de las instantáneas, clonar y seleccionar exactamente las revisiones de FUENTES.md.
4. En PowerShell, compilar el puente: ./build-native.ps1 -ToolsPath C:/herramientas -FfmpegDirectory C:/engine-build/prefix -RuntimeDirectory C:/bridge. native/stage-engine.py resuelve las importaciones PE desde prefix/bin y MSYS2/ucrt64/bin, excluye Windows y recoge avisos/licencias. Ejecutarlo con --help para los parámetros. Colocar libmpv-2.dll en la raíz del proyecto y las otras DLL en runtime/.
5. Generar las muestras sintéticas con ./verificacion/tests/generate-fixtures.ps1 -Ffmpeg C:/ruta/ffmpeg.exe. Para el paquete de fuentes no se publican vídeos; las muestras se crean localmente. ./build.ps1 -ToolsPath C:/herramientas compila y prueba el programa. ./build-portable.ps1 genera y valida el ZIP portátil.

Las cabeceras e import libraries de native/vendor/ffmpeg corresponden al FFmpeg compartido fijado. Si se cambia esa revisión, actualizar también el SDK desde prefix/include y prefix/lib antes de compilar el puente. build.ps1 recompila el puente, pero no reconstruye automáticamente el motor.

fuentes/cargo-vendor permite usar Cargo sin descargar crates. Por ejemplo, pasar --offline y las opciones --config 'source.crates-io.replace-with="vendored-sources"' y --config 'source.vendored-sources.directory="C:/fuentes/cargo-vendor"'. Incluye los cuatro proyectos Cargo y sus dependencias, también de otros destinos. fuentes/rust-upstreams conserva las instantáneas/archivos de bloqueo de librsvg, libdovi y rav1e; librsvg incorpora la preparación windows-sys 0.61.2 -> 0.60.2 indicada por su PKGBUILD.

## Otros sistemas

cargo test --locked --lib comprueba únicamente el núcleo independiente de Windows. No genera aplicaciones para macOS/Linux/Google TV. La ejecución previa de 0.2.7 quedó bloqueada por facturación/límite de gasto. La ejecución 40526609437 de 0.2.8 terminó correctamente: 23 pruebas del núcleo en macos-latest y 23 en ubuntu-latest. No valida las interfaces o reproducción de Windows en esos sistemas. El ZIP de fuentes se preparó antes de esta comprobación y conserva la referencia histórica al bloqueo; esta nota actualiza ese resultado.

El recurso assets/windows.manifest activa los diálogos nativos con botones grandes. build.rs comprueba sus cambios. Las pruebas actuales incluyen comparación de píxeles durante cambios de gafas, inversión, 2D y salida Full SBS.

El generador prepara las muestras necesarias para cargo test, con patrones de color distintos por ojo para detectar shaders obsoletos. Las dos plantillas ASS están incluidas en las fuentes. La batería ampliada de verificacion/ requiere además sus muestras específicas y los medios de usuario autorizados; no se incluyen películas ni ISO. ENIGMAX_TEST_FIXTURES permite verificar la integración con una carpeta de muestras recién generadas.

### Pruebas de audio de 0.2.14

Antes de ejecutar las pruebas, genera la matriz sintética con `python verificacion/tests/generate-audio.py RUTA_A_FFMPEG.exe`. No utiliza archivos originales. `build.ps1` ejecuta la prueba de recursos GDI por separado para evitar interferencias con los tests GPU concurrentes.

### Subtítulos estereoscópicos de 0.2.16

La instantánea mpv incluida contiene el parche nativo documentado en native/mpv-subtitle-stereo.patch sobre la revisión base fijada. Al partir de Git limpio, aplícalo con git apply antes de compilar; build-engine.sh admite exclusivamente ese diff local. No se sustituye un controlador gráfico.

Después de generate-fixtures.ps1, ejecutar python verificacion/tests/generate-stereo-subs.py --ffmpeg RUTA_A_FFMPEG.exe para generar las muestras originales PGS/SRT/ASS y el título MKV de prueba. Las mediciones de profundidad se ejecutan con cargo test; no requieren películas del usuario.

package-sources.py admite --reuse-dependencies y --reuse-sha256 para reutilizar por renombrado un paquete de dependencias sin cambios, tras verificar hash, manifiesto y CRC; evita duplicar temporalmente más de un GiB.
