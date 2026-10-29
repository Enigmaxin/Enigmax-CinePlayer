# Fuentes correspondientes de 0.2.18

El motor utiliza compilaciones propias compartidas de mpv y FFmpeg. Las otras DLL proceden de paquetes UCRT64 de MSYS2 validados con PGP. El inventario exacto, versiones y SHA-256 están en licencias/engine-manifest.txt; los avisos de terceros, en licencias/third-party-runtime.txt y las licencias individuales.

La misma release ofrece gratuitamente dos paquetes adicionales, innecesarios para ejecutar el reproductor:

- Enigmax-CinePlayer-Sources-0.2.18.zip: código del reproductor y puente MVC/UDF, instantáneas completas de mpv/FFmpeg, scripts de compilación y fuentes vendorizadas de las dependencias Cargo del reproductor, libdovi, rav1e y librsvg, con sus archivos de bloqueo.
- Enigmax-CinePlayer-Dependency-Sources-0.2.18.zip: archivos fuente MSYS2, recetas PKGBUILD, parches, metadatos y hashes. Incluye un superconjunto de dependencias de compilación; no todas se cargan al reproducir.

Entrega local 0.2.18: los dos ZIP se preparan junto al paquete portátil. Esta versión todavía no se ha publicado; se publicará cuando lo solicite el usuario. El motor conserva las revisiones base de la release pública v0.2.8 y añade el parche local de subtítulos documentado en native/mpv-subtitle-stereo.patch. Las instantáneas incluidas contienen exactamente esas modificaciones.

Verifica los SHA-256 publicados antes de extraer. Cada paquete es un ZIP independiente; extrae ambos en la misma carpeta para obtener fuentes/ y dependency-sources/. No son volúmenes de un archivo dividido. Consulta fuentes/EnigmaxCinePlayer/COMPILAR.md.

Revisiones del motor:

- mpv: dc4b285c93043d99492c3664a24f4ae08cdfdf0d
- FFmpeg: 6882969ce9fd2aa6aa1d0008b2bfe1c838aac11d

Se incluyen recetas y fuentes; no se afirma reproducibilidad binaria bit a bit ni una auditoría jurídica independiente. Los compiladores y las bibliotecas del sistema operativo no se incluyen en el paquete portátil. El ejecutable no tiene firma Authenticode.
