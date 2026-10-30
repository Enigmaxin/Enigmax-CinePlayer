# Dependencias de 0.2.21

Código propio GPL-3.0-or-later. Rust 1.99.1 GNU x64 y w64devkit 2.10.1/GCC 16.2.0 para el ejecutable y puente; MSYS2 UCRT64/GCC 16.2.0, Meson 1.12.1 y Ninja 1.13.2 para el motor. No se requieren para ejecutar.

- mpv dc4b285c93043d99492c3664a24f4ae08cdfdf0d: compilación propia compartida con Lua 5.1, D3D11, WASAPI, libass, libplacebo, Blu-ray y gestión de color. JavaScript, VapourSynth y SMTC no se habilitan.
- FFmpeg 6882969ce9fd2aa6aa1d0008b2bfe1c838aac11d: compilación propia GPL v3 compartida. Motor y puente utilizan el mismo conjunto FFmpeg, sin otro FFmpeg estático dentro de libmpv. D3D11VA/D3D12VA/DXVA2 y selección de CPU en ejecución. Argumentos completos en native/build-engine.sh. Whisper y libmysofa no se habilitan.
- Otras DLL: paquetes UCRT64 de MSYS2, con versiones fijadas y binarios validados mediante PGP por pacman. Se distribuye la clausura de importaciones PE de 125 DLL, excluyendo las DLL del sistema Windows. Versiones, paquetes propietarios y hashes en licencias/engine-manifest.txt; avisos y textos en licencias/third-party-runtime.txt y las licencias individuales.
- edge264-mvc API 2, fuentes BSD conservadas en native/vendor. Base x86-64 y variantes x86-64-v2/v3 seleccionadas en ejecución. Cuatro hilos para el puente. libudfread LGPL 2.1+, fuentes conservadas en native/vendor. La ISO se abre en lectura.
- Actualizador: semver, serde_json y tempfile, con dependencias fijadas en Cargo.lock. PowerShell 5.1, .NET y Windows Forms del sistema realizan la consulta HTTPS y el auxiliar. No hay servicio ni runtime asíncrono residente.

Las revisiones de mpv y FFmpeg se comprobaron contra Git antes de compilar. Se verificó la diferencia CRLF/LF del checkout Windows sin cambios de contenido. El identificador mpv puede contener «dirty» por esos finales de línea; no corresponde a un parche del motor.

Los archivos fuente MSYS2 proceden del repositorio oficial. El SHA de cada PKGBUILD se contrastó con BUILDINFO del paquete binario validado con PGP. Los archivos fuente no se presentan como firmados individualmente: el inventario registra URL y SHA-256. Las recetas, parches, instantáneas y fuentes Cargo se incluyen en los paquetes adicionales de la misma release: FUENTES.md. No se afirma una reconstrucción bit a bit ni una auditoría jurídica independiente.

Orígenes:

- https://github.com/mpv-player/mpv
- https://github.com/FFmpeg/FFmpeg
- https://repo.msys2.org/mingw/sources/
- https://github.com/msys2/MINGW-packages
- https://github.com/jens-duttke/edge264-mvc
- https://code.videolan.org/videolan/libudfread
- https://github.com/skeeto/w64devkit

Los coeficientes Dubois siguen la referencia stereo3d de FFmpeg. ASS 3D utiliza libass y la fusión antes del shader. Las pruebas y límites se documentan en VERIFICACION_0.2.8.md; no se certifica cualquier archivo, CPU, TV HDR o receptor HDMI.

0.2.9 conserva el mismo motor y las mismas dependencias que 0.2.8; no ha sido necesario reconstruir mpv/FFmpeg. El EXE añade un manifiesto de Common Controls v6 para los diálogos nativos incluidos en Windows 11. No se añade una DLL distribuida.

0.2.11 recompila el puente cine_mvc.dll con la lectura alternativa de flujos UDF verificados. No añade bibliotecas de descifrado ni cambia las revisiones fijadas de mpv/FFmpeg. Las nuevas funciones usan las mismas bibliotecas FFmpeg ya distribuidas.

0.2.16 recompila mpv con el parche local native/mpv-subtitle-stereo.patch: proyección y profundidad de la geometría de subtítulos PGS/SRT/ASS, conservando sus mapas de píxeles y transparencia. El SHA-256 del parche y del motor modificado constan en licencias/engine-manifest.txt. FFmpeg y las otras DLL no cambian.

0.2.17 conserva el motor y sus dependencias. La aplicación fija la profundidad añadida de subtítulos a cero y retira sus controles; la propiedad interna del motor no se expone al usuario.

0.2.18 conserva las mismas DLL y dependencias de reproducción; modifica únicamente la aplicación y el auxiliar de actualización.

0.2.19 mantiene las dependencias de reproducción de 0.2.18. Se recompilan la aplicación y sus diálogos nativos; no se incorpora un motor ni un paquete de códecs adicional.

0.2.20 conserva las revisiones del motor y las dependencias Cargo de 0.2.19. Se recompila la aplicación y el puente nativo habitual; no se añade ningún códec ni servicio residente.

0.2.21 mantiene mpv/FFmpeg y dependencias Cargo. Recompila el puente cine_mvc.dll para IO cancelable Windows y consulta explícita de metadatos; conserva la estructura EC_Info histórica. No añade DLL de terceros.
