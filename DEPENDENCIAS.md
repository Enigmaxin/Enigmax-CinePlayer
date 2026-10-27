# Dependencias de 0.2.7

Código propio GPL-3.0-or-later; Rust 1.99.1 GNU x64 y w64devkit 2.10.1/GCC 16.2.0. Perfil release LTO. No se necesitan para ejecutar.

- libmpv estándar x64: v0.41.0-1227-ge471496c5, API 2.6, shinchiro 20261021. DLL conservada de 0.1.1; SHA-256 04E4D27DC385DC18DDD93FBE69C75E635B864F3C6EC53A9122DF54C2F7842F4A.
- FFmpeg compartido BtbN n9.0/latest GPL, descarga local del 26/10/2026. avformat-63, avcodec-63, avutil-61 y swresample-7; se carga al catalogar ISO o decodificar MVC. No se lanza un proceso FFmpeg para reproducir. El motor libmpv incorpora su propio FFmpeg estático.
- edge264-mvc API 2, fuentes BSD descargadas de main el 26/10/2026 y conservadas en native/vendor. Compilado base x86-64 con variantes x86-64-v2/v3 seleccionadas en ejecución. Cuatro hilos para el puente. No se modificaron las fuentes del decodificador.
- libudfread, fuentes master descargadas el 26/10/2026, LGPL 2.1+, conservadas en native/vendor. El bloque de entrada propio abre el archivo en lectura mediante AVIO y serializa la lectura de sectores de la ISO.
- Pruebas: FFmpeg estático git 9a861e968; conformance_check del decodificador. Python/Pillow/numpy se utilizan fuera del reproductor para QA.

Los hashes de los archivos entregados están en SHA256SUMS.txt. Son hashes locales de identificación, no firmas independientes del proveedor. Las instantáneas de fuentes incluidas fijan el contenido usado; main/master no son números de publicación estables.

Orígenes y documentación:

- https://mpv.io/installation/
- https://mpv.io/manual/master/
- https://github.com/shinchiro/mpv-winbuild-cmake/releases/tag/20261021
- https://github.com/BtbN/FFmpeg-Builds/releases
- https://github.com/jens-duttke/edge264-mvc
- https://code.videolan.org/videolan/libudfread
- https://github.com/skeeto/w64devkit/releases/tag/v2.10.1

Los coeficientes Dubois y el filtro alternativo siguen la referencia stereo3d de FFmpeg. Se adjuntan licencias de mpv, FFmpeg, edge264-mvc y libudfread. Esta entrega local no contiene el código fuente completo correspondiente a todos los componentes del binario libmpv/FFmpeg; antes de una redistribución pública hay que preparar ese conjunto y revisar sus obligaciones. No se presenta como paquete público auditado.

La ampliación ASS 3D utiliza el libass integrado en libmpv y la API de propiedades observadas. No añade crates ni DLL nuevas. Se consultaron el manual y el código de la versión mpv e471496c56 para comprobar la fusión antes de MAIN y la corrección de píxel ASS:

- https://mpv.io/manual/master/#options-blend-subtitles
- https://github.com/mpv-player/mpv/blob/e471496c56/sub/sd_ass.c
- https://github.com/mpv-player/mpv/blob/e471496c56/video/out/gpu/video.c

Actualizador: crates semver (MIT/Apache-2.0), serde_json (MIT/Apache-2.0) y tempfile (MIT/Apache-2.0), con versiones transitivas fijadas en Cargo.lock. Windows PowerShell 5.1, .NET y Windows Forms del sistema realizan la consulta HTTPS y el auxiliar; no se añaden DLL distribuidas. El código de consulta no utiliza Rust async ni un runtime residente.
