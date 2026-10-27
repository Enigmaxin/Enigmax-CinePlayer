# Preparación de Google TV

Esta entrega solo ejecuta Windows x64. No hay APK ni pruebas Android.

El núcleo Rust (`src/lib.rs`, `src/stereo.rs`, `src/ass3d.rs`) contiene geometría, perfiles validados y generación de matrices/shaders sin dependencias de Windows, GUI ni decodificadores. Puede probarse con `cargo test --lib`. El workflow portable-core prepara comprobaciones Linux/macOS; no se ha ejecutado en servicios remotos.

Una futura interfaz Android puede reutilizarlo mediante JNI y adaptar la presentación a libmpv o a una ruta MediaCodec/Surface. Las funciones específicas de Windows permanecen en el ejecutable y en el puente C. edge264-mvc dispone de objetivos Android, pero el puente utiliza SRWLOCK, API de archivos Win32 y temporizador Windows: debe portarse antes de compilarlo con NDK. Las bibliotecas compartidas Windows no sirven en Android.

Trabajo pendiente: mando y foco de TV, SAF/USB, permisos y ciclo de vida, JNI, ABI arm64-v8a, empaquetado/licencias, renderizador y shaders, HDR por dispositivo, salida AudioTrack/passthrough, ISO/UDF con lectura aleatoria y medición del coste MVC por software. También debe validarse el acceso a ambas vistas, A/V, PGS/ASS, temperatura y reproducción sostenida. No se heredan las garantías de una TV de las pruebas realizadas en este PC.

La detección de parejas ASS y sus perfiles están en el núcleo portátil. La composición conjunta actual depende de las etapas MAIN de mpv y de su ruta gpu; el frontend Google TV tendrá que proporcionar una ruta equivalente o validarla en su propio motor. No hay pruebas Android de esta función.
## Ampliaciones portables de 0.2.2

`src/control_layout.rs` define la geometría, las zonas de interacción y la proyección mono/anaglifo/SBS/OU sin dependencias de Windows. Puede reutilizarse al implementar la interfaz Android; `src/controls.rs` rasteriza actualmente con GDI y necesita un reemplazo Android. El código Lua actual conecta eventos de libmpv en Windows. La detección conservadora del nombre/metadatos está en `src/stereo.rs`; la pregunta de visualización usa menús Win32 y debe adaptarse al mando de Google TV. Estas piezas no constituyen una aplicación Android compilada.

