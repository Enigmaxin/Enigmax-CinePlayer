# Verificación local de Enigmax CinePlayer 1.0.6

Fecha: 12/11/2026. Entrega local, sin publicación en GitHub.

## Título duplicado: causa confirmada y corrección

Archivo autorizado: `D:\3D H265 4K MKV\IMAX; Mars 3D (2007) 3D-SBS 720p.mkv`.
FFprobe obtiene un TITLE, pero el demultiplexor Matroska de mpv devuelve
`IMAX: Mars 3D (2007) / IMAX: Mars 3D (2007)`. Su código concatena el título
del segmento y la etiqueta global TITLE mediante ` / `. No era un segundo
dibujo del HUD ni un defecto del modo anamórfico.

La presentación elimina repeticiones completas e idénticas del título MKV.
Conserva títulos distintos separados por barras, mayúsculas y nombres literales.
HUD y barra de título usan el mismo valor normalizado. El contenido de los
metadatos y el archivo original no se escriben.

La prueba específica pasó en SBS anamórfico, SBS original, anaglifo y 2D.
Se verificaron tamaño y fecha del original antes y después; se guardaron capturas.
También hay pruebas de Unicode, títulos distintos, barras legítimas y truncamiento.

## Mejoras internas inspiradas en funciones de renderizadores de alta calidad

No se integra madVR ni se cambia el motor. madVR PC es un renderizador DirectShow;
madVR Envy es un procesador diferente. No se atribuyen al primero funciones
exclusivas del segundo. La aplicación conserva libmpv/libplacebo y D3D11.

| Perfil | Luminancia | Crominancia explícita | Antirringing |
|---|---|---|---|
| Máxima calidad, gpu-next | ewa_lanczossharp | ewa_lanczos | 0,6 |
| Máxima calidad, gpu heredado | ewa_lanczossharp | ewa_lanczossharp | 0 |
| Equilibrado | spline36 | spline36 | 0,6 |
| Compatibilidad | spline36 | spline36 | 0 |
| Mínimo consumo | bilinear | bilinear | 0 |

El escalado cromático deja de depender implícitamente del escalado de luminancia.
Las opciones se restablecen al cambiar de perfil o renderizador, sin arrastrar
los ajustes costosos al perfil de bajo consumo. Se conservan aceleración,
límites de memoria y tratamiento HDR existentes. No se añaden transformaciones
tonales a SDR ni interpolación temporal. No se incorporan controles ICC/LUT,
debanding ni recuperación de contraste sin validación específica.

En patrones YUV de 10 bits, comparados con una referencia 4:4:4, la suma de
excursiones fuera de los niveles de meseta se redujo: luminancia de 196 a 78
en Alta y de 208 a 69 en Equilibrado; crominancia de 341 a 194 en Alta.
Son medidas de ese patrón y línea de muestreo, no una puntuación universal
de calidad ni una demostración de mejora de cualquier película.

En dos recorridos de orden inverso con la misma fuente y tamaño de ventana,
las medianas de las pasadas GPU UHD HDR10 fueron aproximadamente 1,94 -> 2,77 ms
en Alta y 1,01 -> 1,29 ms en Equilibrado. Por tanto, la mejora tiene un coste
gráfico medido y no se presenta como una reducción de consumo. Las ventanas
medidas fueron 1429x804 para UHD y 714x804 para el MKV 3D, constantes dentro
de cada comparación. No es una medición de salida 4K física.

No hubo descartes en las muestras con los ajustes nuevos de esa comparación.
El primer recorrido SDR de referencia tuvo un descarte. Las mediciones son
cortas, dependientes de equipo/carga y no certifican otros ordenadores.

## Verificaciones ejecutadas

- `cargo fmt --check`: correcto.
- `cargo clippy --offline --locked --all-targets -- -D warnings`: correcto.
- Batería completa, secuencial: 57 pruebas de biblioteca y 69 de aplicación
  aprobadas; 0 fallidas. 16 pruebas especiales omitidas por sus requisitos.
- Prueba específica con el MKV del usuario: 1 seleccionada y aprobada.
- Las 153 transiciones de calidad conservaron pistas, metadatos de color,
  dimensiones de fuente, posición y funcionamiento del reproductor.
- La batería incluye geometría estéreo, subtítulos ASS/PGS/SRT, cambios de
  modo, sincronía, cierre, pausas, búsqueda y conversión HDR a SDR por patrones.
- Compilación release correcta; recursos PE y Cargo indican 1.0.6.
- ZIP portátil: CRC, manifiesto, tamaños y SHA-256 de sus 141 archivos correctos.
  Extracción limpia con PATH limitado al sistema; cuatro pruebas de reproducción
  terminaron con código 0 y captura: Full-SBS sintético, Mars anamórfico,
  Mars original y HDR10 UHD sintético.
- En las tres muestras temporizadas del ZIP: cero esperas de caché y cero
  descartes de vídeo/decodificador. Mars utilizó WASAPI; el patrón UHD no tiene
  audio. No equivale a una prueba auditiva de todos los formatos.
- Las DLL no se actualizan. Cargo.lock solo cambia la versión de la aplicación.

## Límites y observaciones pendientes

Cambiar de perfil no siempre es instantáneo: el máximo de aplicación observado
fue 1513 ms al volver a Máxima calidad desde una ruta de anaglifo distinta.
La entrada en Mínimo consumo tuvo hasta 1032 ms y 11 descartes en una transición.
La reproducción continuó y las aserciones de conservación pasaron, pero esto
impide afirmar que todas las transiciones sean imperceptibles. Algunos contadores
se reinician al cambiar de ruta; sus diferencias negativas no son descartes reales.
No se demuestra aquí que esas pausas sean nuevas ni se modifica su arquitectura.

Siete archivos de configuración conservan su SHA-256 inicial. `last-video.json`
y `resume.json` cambiaron durante el intervalo de trabajo; se conservan sus
valores actuales, sin sobrescribirlos con una reconstrucción. La comparación
de hashes no permite atribuir por sí sola el origen de esos dos cambios.
Las nuevas pruebas de estado utilizan directorios temporales y la prueba del
ZIP utiliza una extracción temporal independiente.

No se han certificado en esta tanda la salida física HDR, Quest 3/Virtual Desktop,
MVC sobre ISO real, pantallas calibradas ni todos los controladores de GPU.
Una batería aprobada no demuestra ausencia total de errores.

Evidencias completas: `work/validacion-1.0.6/` del proyecto.
Documentación técnica consultada: https://mpv.io/manual/master/ y las opciones
del motor fijado incluido en el proyecto; https://libplacebo.org/options/ .
