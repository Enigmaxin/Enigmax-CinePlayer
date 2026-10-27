# Compilar desde este repositorio

Código de Enigmax CinePlayer 0.2.7, GPL-3.0-or-later. Las DLL de reproducción y los ajustes personales no forman parte del árbol Git. Las licencias de terceros se conservan en licencias/ y en native/vendor/.

## Windows x64

- Instalar Rust GNU x64 y MinGW-w64 con GCC, make y windres. El código local se verificó con Rust 1.99.1 GNU. No se requiere ese entorno para ejecutar el reproductor.
- Disponer de libmpv-2.dll en la raíz y las DLL FFmpeg y cine_mvc.dll en runtime/. El paquete portátil completo contiene estos archivos; no basta el EXE. Las procedencias y versiones están documentadas en DEPENDENCIAS.md.
- Para las pruebas del ejecutable se generan localmente cuadros negros, ondas sinusoidales y subtítulos de prueba. No se publican películas ni vídeos de prueba en Git:

```powershell
./verificacion/tests/generate-fixtures.ps1 -Ffmpeg 'C:/ruta/ffmpeg.exe'
./build.ps1 -ToolsPath 'C:/ruta/herramientas'
./build-portable.ps1
```

ToolsPath admite la disposición cargo/bin, rustup y w64devkit/bin usada por los scripts. También pueden usarse Rust y MinGW ya presentes en PATH. La compilación verifica formato, Clippy y pruebas antes de generar el EXE. Las cabeceras, bibliotecas de importación necesarias y fuentes del puente están incluidas; no se incluyen los compiladores ni las DLL del motor en Git.

En Linux y macOS se comprueba solamente el núcleo independiente de la plataforma:

```sh
cargo test --locked --lib
```

Eso no produce todavía aplicaciones de escritorio para esas plataformas.

## Distribución de binarios

La validación funcional local no certifica por sí sola la distribución pública de todas las dependencias. La documentación de esta entrega mantiene pendiente reunir el código fuente correspondiente de todos los componentes incluidos en libmpv/FFmpeg. No se afirma que unos enlaces a ramas main/master sean un paquete de fuentes correspondiente completo. Véase DEPENDENCIAS.md.