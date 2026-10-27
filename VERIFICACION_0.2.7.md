# Auditoría Enigmax CinePlayer 0.2.7 — 27/10/2026

Se ha trabajado en la única base vigente. Cargo.toml, recursos PE y EXE indican 0.2.7. Entrega: Enigmax-CinePlayer-Portable-0.2.7-windows-x64.zip, 104.401.963 bytes. No se ha generado un instalador ni publicado en GitHub. La comprobación, descarga y sustitución se ejecutan fuera del hilo de reproducción.

## Defectos corregidos y cambios

- El actualizador antiguo seleccionaba Setup y convertía una copia portátil en instalación. Ahora exige el ZIP portátil versionado, sin registro ni asociaciones, con EXE, bibliotecas y recursos compatibles como conjunto.
- La descarga hacía lecturas de red bloqueantes para su ventana. Ahora atiende eventos durante conexión y lectura y permite cancelar; se han mantenido límites de tiempo y tamaño. La consulta JSON tiene también un límite total de lectura.
- El paquete se valida antes de cerrar: SHA-256 de GitHub, manifiesto, hash y tamaño de cada archivo, límites de expansión, arquitectura x64 y versión PE. Se rechazan rutas ajenas, duplicados, enlaces y uniones. Configuración y medios no son destinos permitidos.
- Sustitución recuperable en el mismo volumen, bloqueo exclusivo y recuperación probada ante DLL bloqueada. El nuevo manifiesto permite retirar archivos propios obsoletos. Una recuperación incompleta conserva su carpeta y explica cómo localizarla. No se promete atomicidad ante cortes eléctricos.
- Una posición guardada para un cambio de transporte podía quedar pendiente si se abría otro archivo antes de consumirla. new_open la descarta; se ha añadido una regresión específica.
- Quedaban diálogos modales en errores de apertura, presentación y ASS dentro del consumidor de eventos. Se sustituyen por el diagnóstico acotado y aviso OSD utilizados por los controles. Así esos errores no esperan que el usuario cierre un diálogo para consumir más eventos.
- Fallar al asignar un icono ya no termina la reproducción normal. Las pruebas siguen tratando ese error como fallo verificable.
- Comprobaciones de Clippy y pruebas de build.ps1 fijadas a Cargo.lock. Se han mantenido los motores, códecs, colas de búsqueda, renderizado de audio y temporización originales; no se han cambiado límites de memoria por estimaciones sin pruebas.

## Verificación ejecutada

Equipo: Intel Core i9-9900K, Windows 11 Home, compilación 26300. Una sola configuración real; no es una certificación para todos los Intel o GPU.

| Comprobación | Resultado |
| --- | --- |
| Formato Rust, Clippy all-targets con -D warnings y compilación release | Correctos |
| Pruebas Rust | 23 de biblioteca + 5 del EXE correctas; 1 MVC omitida por defecto y ejecutada aparte |
| MVC: 100 búsquedas rápidas y cierre durante decodificación | Correcto con TALKTOME e Hyperspace autorizadas; cierres de 491,64 y 244,25 ms |
| Puente C propio: GCC -fanalyzer, compilación y prueba Annex B | Sin advertencias del analizador; compilación y prueba correctas |
| Regresión de vídeo, MP4, 4K HDR sintético, SBS/OU, anaglifo CPU/GPU, SRT/ASS | 22 casos correctos |
| Regresión ASS 3D, gafas, intercambio de ojos, forzado y salida original/2D | 24 casos correctos |
| Paquete/recuperación en Windows PowerShell 5.1 | 21 comprobaciones correctas |
| Auxiliar integrado: análisis sintáctico y rechazo de destino HTTPS externo | Correctos |
| ZIP final extraído en carpeta limpia: MP4 anaglifo y Blu-ray MVC anaglifo | Ambos correctos; EXE idéntico al vigente; carpeta de prueba eliminada |
| OSV: versiones fijadas en Cargo.lock | 25 dependencias consultadas, sin avisos devueltos; respuesta guardada en osv-crates.json |

La prueba de paquete bloquea una DLL con FileShare.Read, provoca un error de sustitución y comprueba la restauración y sus hashes. También comprueba conservación de config y un archivo ajeno, retirada de DLL propia obsoleta, archivo alterado, versión distinta, rutas relativas/absolutas peligrosas y unión NTFS. No se ha simulado un corte de energía.

Se comprobó visualmente con la habilidad computer-use la apertura del menú contextual, su cancelación y reapertura y la apertura de Audio. El diálogo de archivos no fue manejado de forma fiable por la automatización; se cerró y la muestra se abrió por línea de órdenes. No se presenta como una validación manual completa de todas las opciones. Las rutas de botones y el ciclo de menú también tienen regresiones Lua y de geometría con libmpv real. No se automatizó una publicación ni se cambiaron ajustes de Windows.

## Sincronización y memoria: siete sesiones de 30 segundos

Audio WASAPI real, con volumen cero, pausas, búsquedas y cambios 2D/anaglifo. La columna A/V excluye el primer segundo de cada transición, pausa y búsqueda. El criterio automatizado fue menos de 100 ms, al menos 20 muestras estables y ningún error de subtítulo sintético. No representa escucha humana ni prueba prolongada.

| Caso | Máximo A/V estable (ms) | Fotogramas descartados acumulados | Pico memoria privada (MiB) |
| --- | ---: | ---: | ---: |
| H.264 / AC3 / SRT | 0,049 | 0 | 114,7 |
| MP4 Full SBS / AAC / SRT externo | 8,427 | 1 | 210,8 |
| HSBS / EAC3 / ASS 3D | 0,036 | 0 | 138,3 |
| HEVC 10 bits / AAC | 0,040 | 0 | 121,4 |
| H.264 VFR / AC3 | 0,046 | 0 | 116,5 |
| ISO TALKTOME MVC anaglifo | 0,250 | 0 | 915,4 |
| Titanic HSBS / ASS | 0,859 | 1 | 380,1 |

Los siete casos cumplen el criterio. Se comprobaron automáticamente 437 situaciones de texto/tiempo en las muestras sintéticas, sin errores. Las dos fuentes reales no tienen referencia textual automatizada: no se certifica su sincronía lingüística original. Los picos de memoria incluyen las transiciones; no son consumos permanentes ni máximos garantizados. No se ha identificado una fuga por estos datos y no se ha realizado una prueba de varias horas. Hay un descarte en MP4 y otro en Titanic; no se afirma cero descartes.

La primera ejecución del arnés se detuvo porque una variable de subtítulo externo vacía se transmitió como valor definido en PowerShell 7. Se corrigió el arnés, se conservó su CSV parcial en sync-harness-empty-env y se repitió completa la batería con éxito. No se ha ocultado como fallo de reproducción corregido.

## Límites y pendientes

- La consulta automática real devolvió HTTP 403, registrado sin interrumpir la reproducción. No se atribuye a una causa definitiva. No se ha probado descarga y actualización real desde una release nueva: hace falta publicar el ZIP compatible y poder consultar la API. Las versiones anteriores que solo aceptaban Setup necesitarán descargar manualmente el primer ZIP portátil.
- SHA-256 verifica correspondencia con el asset y no sustituye una firma digital. El EXE no está firmado. Las pruebas OSV cubren los crates fijados, no todos los componentes internos de libmpv/FFmpeg ni todos los fallos desconocidos.
- Se conservan los límites documentados de HDR10+/Dolby Vision, frame packing HDMI, discos cifrados, BD-J y autorías MVC/ASS inusuales. No se ha ensayado una cadena física HDR/HDMI/passthrough ni macOS, Linux o Google TV.
- La exploración ISO y la preparación inicial MVC siguen siendo llamadas síncronas del controlador; el puente limita la decodificación a 25 segundos por operación. Estas pruebas no certifican la latencia de apertura de discos lentos o dañados. No se ha introducido una refactorización de concurrencia sin demostrar una mejora segura.
- Los binarios multimedia de terceros no se reconstruyeron. Antes de distribución pública se mantiene la revisión de código fuente correspondiente y obligaciones descrita en DEPENDENCIAS.md.

## Evidencias y limpieza

verificacion/0.2.7/ contiene package-tests.log, osv-crates.json, sync/results.json, CSV y capturas, las regresiones playback/ y ass3d/ y las dos capturas portable-*. Los registros de compilación y análisis se han copiado aquí. Los medios originales se han abierto únicamente en lectura.

Se eliminaron los paquetes 0.2.5 y el árbol regenerable work/build con ejecutables de compilación y pruebas. Se conservaron herramientas, fuentes, dependencias, ajustes y medios. Liberados 843,3 MiB según el inventario LIMPIEZA-VERSIONES-0.2.7.json. Se conserva la carpeta vigente y un único ZIP vigente con su SHA-256.