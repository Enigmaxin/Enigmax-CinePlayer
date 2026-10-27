# Verificación de 0.2.8 — motor desde fuentes fijadas

Pruebas locales en Windows 11 x64 del 27/10/2026. Sustituye el motor de 0.2.7 por compilaciones propias de las revisiones mpv/FFmpeg indicadas en FUENTES.md. Las otras bibliotecas son paquetes MSYS2 UCRT64 firmados; no se afirma haber recompilado cada dependencia secundaria.

## Comprobaciones ejecutadas

- Compilación C del puente y prueba Annex B: correctas.
- cargo fmt, Clippy --all-targets con -D warnings, 23 pruebas del núcleo y 5 del ejecutable: correctos. Una prueba MVC se omite en la ejecución habitual y se ejecutó explícitamente con TALKTOME y Hyperspace: ambas correctas, incluyendo búsquedas y destrucción del decodificador al cerrar.
- 22 casos generales de renderizado: correctos. Incluyen 4K HDR10/conversión SDR, MP4, selección de ojo, variantes anaglifas GPU/CPU, SRT y ASS.
- 24 casos ASS 3D: correctos. El análisis de píxeles también confirmó geometría, disparidad, gafas, intercambio y convergencia.
- 28 comprobaciones de paquete y recuperación en Windows PowerShell 5.1: correctas. Incluyen bloqueo de DLL/recuperación, conservación de ajustes/archivo ajeno, retirada de DLL propia obsoleta, rutas/dispositivos peligrosos, límites de 256 archivos y nombres de dependencias con + o puntos.
- ZIP portátil extraído y validado en una carpeta nueva: MP4 Full SBS anaglifo y TALKTOME MVC anaglifo reproducidos correctamente. Los 141 archivos del manifiesto conservaron sus hashes tras reproducir.
- Dos ZIP de fuentes: verificación CRC completa y SHA-256. El inventario comprobó que cada DLL de paquete MSYS2 tiene su archivo fuente correspondiente. 127 archivos fuente MSYS2 y 647 versiones de crates vendorizadas (incluye destinos/dependencias no usados en ejecución). No se incluyen películas, ISO ni configuraciones personales del reproductor.

## Sincronización

Siete sesiones de 30 segundos con WASAPI real silenciado, pausas, búsquedas y cambios de modo. Se excluyen de A/V estable el primer segundo de cada transición, búsquedas, pausa y espera de caché. Criterio: máximo inferior a 100 ms, más de 20 muestras estables y cero errores de subtítulos sintéticos.

| Caso | Máximo A/V estable (ms) | Descartes acumulados | Pico privado (MiB) |
| --- | ---: | ---: | ---: |
| h264-ac3-srt | 0.027 | 0 | 149.7 |
| mp4-aac | 7.593 | 0 | 241.6 |
| hsbs-eac3-ass | 0.016 | 0 | 165.9 |
| hevc10-aac | 0.026 | 0 | 157.4 |
| vfr-ac3 | 38.087 | 0 | 156.3 |
| mvc-talktome | 0.016 | 0 | 907.8 |
| titanic-hsbs-ass | 1.067 | 1 | 392.5 |

Resultado: siete casos dentro del criterio; 437 comprobaciones de subtítulos sintéticos y cero errores. No representa escucha humana, verificación semántica de los subtítulos reales ni reproducción prolongada.

El portátil comprimido ocupa 72.040.908 bytes frente a 104.401.963 de 0.2.7. La memoria privada de las muestras sencillas ha aumentado respecto a la auditoría anterior; no se afirma una reducción global del consumo. El FFmpeg compartido evita duplicar el motor en libmpv y el puente, pero las bibliotecas compartidas tienen su propio coste. El pico MVC durante cambios de modo fue 907,8 MiB; no es un límite garantizado.

## Fuentes y límites

Se verificaron revisiones Git y ausencia de cambios de contenido, teniendo en cuenta CRLF/LF. Cada receta PKGBUILD de las fuentes MSYS2 coincide con el SHA registrado en BUILDINFO de su paquete validado con PGP. Los SHA de los tres ZIP subidos coinciden con los digest devueltos por GitHub. Las fuentes no se presentan como firmadas individualmente ni como reconstrucción bit a bit.

El compilador upstream mpv emitió una advertencia sobre el tamaño de memcpy en filters/f_hwtransfer.c; no se demostró un defecto ni se ocultó con un parche. Los análisis y ensayos no prueban ausencia de cualquier bug. No se ejecutaron sanitizadores ni una auditoría jurídica independiente.

GitHub Actions no inició sus trabajos por una restricción de facturación/límite de gasto de la cuenta; no hay resultado de CI de Linux/macOS. No se certifican otros equipos, todos los procesadores Intel, HDR/HDMI físico, passthrough a receptor, Blu-ray cifrado, menús completos ni cualquier archivo. Las limitaciones funcionales de README.md siguen vigentes. No se creó instalador ni se cambiaron asociaciones o el registro.
