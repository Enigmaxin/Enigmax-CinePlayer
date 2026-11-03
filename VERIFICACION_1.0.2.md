# Enigmax CinePlayer 1.0.2 — entrega local, 3 de noviembre de 2026

## Cambios

- Información de archivo, vídeo, audios y subtítulos: solo se muestran propiedades reconocidas. Se omiten los valores ausentes, desconocidos o automáticos. Los metadatos dinámicos PQ no se presentan como prueba inequívoca de HDR10+.
- Ajuste de brillo por ojo: listas de porcentajes con el valor actual marcado, aplicación inmediata y menú abierto. Brillo neutro 100 %, compensación de crosstalk neutra 0 %. Su ámbito sigue siendo anaglifo.
- Tamaño, color y posición de subtítulos: editores persistentes, con actualización del marcado sin recrear la ventana del menú. Se cierran con Escape, clic fuera o pulsando Subs. La selección de pista sigue cerrándose al elegir.
- Tamaño y color continúan limitados a SRT; se conserva la autoría de ASS y los píxeles de PGS. La posición usa el ajuste del motor sin sustituir posiciones ASS explícitas.
- Se mantiene el menú contextual nativo en 2D/anaglifo. Desde sus opciones de aspecto se abre el editor gráfico persistente correspondiente. Las salidas estéreo conservan su proyección por ojo.

No se ha sustituido el motor, unificado radicalmente el compositor ni actualizado ninguna DLL. Las propuestas de cambios profundos siguen sujetas a una evaluación concreta; no se presentan como implementadas.

## Validación funcional

Registros en `work/mejoras-2026-11-03` del proyecto:

- 56 pruebas del núcleo y 58 de aplicación aprobadas, ninguna fallida. Doce pruebas optativas quedaron excluidas en esa pasada; los sondeos posteriores se ejecutaron explícitamente y por separado.
- Recursos GDI: prueba optativa aislada aprobada, 100 ciclos y contador 0 → 1.
- 45 selecciones de calibración y 45 de aspecto de subtítulos en 15 paneles de cada tipo, cubriendo cinco proyecciones. Comprobación de continuidad del panel, marcado y conservación de pistas, filtros, pausa y renderizador.
- 65 selecciones de posición y prueba de información reconocida aprobadas.
- 153 transiciones de calidad aprobadas. El fallo histórico intermitente `gamma: None → auto` conserva causa no demostrada; estas pasadas no lo declaran resuelto.
- Formato Rust y Clippy con `-D warnings` comprobados. La advertencia del sistema de archivos sobre hardlinks de la caché incremental se refiere a la compilación, no al funcionamiento del reproductor.
- Cuatro pruebas C recompiladas y aprobadas: cancelación de lectura, reconstrucción Annex B, detección NAL/MVC y política de selección/restauración de frecuencias.
- Fotogramas SDR y HDR→SDR detenidos en el segundo 12: sus hashes de píxeles RGB coinciden entre 1.0.1 y 1.0.2. Solo demuestra igualdad de esos dos fotogramas de referencia.

## Medición de fluidez

Prueba optativa `quality_audit::steady_playback_pacing_probe`, aislada, con configuración temporal propia: calentamiento de 3 segundos y 16 segundos medidos por escenario; 63–64 muestras, ventana de vídeo de 1920×1080 y pantalla comunicada de 60 Hz.

| Caso | Fuente | Pérdidas de salida | Pérdidas del decodificador | Pausas por caché |
| --- | --- | ---: | ---: | ---: |
| SDR con movimiento | 1920×1080, 60 fps, AVC | 0 | 0 | 0 |
| Full SBS con movimiento | 3840×1080, 24 fps, AVC | 0 | 0 | 0 |
| Full SBS en anaglifo | 3840×1080, 24 fps, AVC | 0 | 0 | 0 |
| Anaglifo y ASS pesado | Igual, 70 eventos animados/desenfocados | 0 | 0 | 0 |
| HDR10 a SDR | UHD, 24 fps | 0 | 0 | 0 |

Se comprobó que el tiempo del vídeo avanzaba con el reloj sin pausas de lectura. Los casos con audio utilizan una salida nula temporizada: sus cifras de sincronización no certifican la latencia de un receptor físico. El caso HDR no contiene audio y no permite medir A/V. La aceleración activa fue D3D11VA; el ASS estereoscópico utiliza la ruta `gpu`, los otros casos `gpu-next`.

Los tiempos de pasos GPU son medidas internas, no equivalen al instante físico de presentación ni al tiempo de todo el pipeline. Se conservan sus muestras originales y no se usa el promedio de CPU como sustituto de fluidez.

### Presentación ETW frente a MPC

Capturas con [PresentMon 2.6.0 oficial](https://github.com/GameTechDev/PresentMon/releases/tag/v2.6.0), sin instalar servicio ni inyectar código. SHA-256 del instrumento: `b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af`. Capturas de 16 segundos dirigidas al PID de cada proceso propio, con entrada de usuario desactivada en la captura. Se analizó el tramo 0,5–15 segundos y se midió un área cliente de ventana de 1280×720 píxeles físicos para ambos reproductores. La barra propia de MPC puede reducir su área de vídeo: esto no equipara el tratamiento de imagen ni constituye una comparación de eficiencia a idéntica calidad.

| Fuente y reproductor | Intervalos de presentación analizados | P99 de intervalo mostrado | Intervalo máximo |
| --- | ---: | ---: | ---: |
| 60 fps, CinePlayer | 870 | 16,9433 ms | 17,0294 ms |
| 60 fps, MPC-HC | 870 | 16,9434 ms | 17,0865 ms |
| 24 fps, CinePlayer | 348 | 50,2454 ms | 50,3390 ms |
| 24 fps, MPC-HC | 348 | 50,1165 ms | 50,2111 ms |
| 24 fps, anaglifo y ASS, CinePlayer | 348 | 50,2726 ms | 50,4335 ms |

En estos tramos no hubo intervalos superiores a 75 ms. Los intervalos alternos de aproximadamente 33,3/50 ms al presentar 24 fps en 60 Hz son coherentes con la cadencia 2:3. Los modos ETW observados fueron Independent Flip en CinePlayer y composición con copia GPU GDI en MPC; no se igualaron sus renderizadores. Cada fila corresponde a una captura corta: no se afirma significación estadística de las pequeñas diferencias ni una equivalencia universal.

Registros: `presentmon-*.csv`, `presentmon-summary.json`, `presentmon-access.log` y `presentmon-integrity.json`. El primer intento de ETW sin permisos fue denegado; la captura permitida posterior se completó sin cambiar grupos de usuarios. Dos intentos de preparación del caso anaglifo utilizaron un identificador de prueba incorrecto; se corrigió únicamente el controlador de prueba a `fsbs-ana` y se completó esa captura por separado. No se interpreta ese fallo del controlador como un fallo de reproducción.

**Decisión:** no se ha demostrado un defecto de fluidez que justifique modificar el motor, alterar la velocidad del contenido o forzar interpolación. Se incorpora el sondeo reproducible como prueba optativa y se conservan los cambios de interfaz ya validados. Este resultado no certifica MVC/ISO real, transición de modos ni sesiones prolongadas: las transiciones se cubren por las pruebas funcionales anteriores, sin medir su presentación física aquí.

## Comparativas y decisión de optimización

- MPC-HC instalado: 2.8.4. Doce ejecuciones iniciales, seis adicionales de CinePlayer 1.0.2 y ocho alternadas 1.0.1/1.0.2. Las muestras de CPU varían y las primeras ventanas/renderizadores no estaban equiparados: no demuestran superioridad general ni un cambio causal de rendimiento.
- Estudio de caché de subtítulos, seis ejecuciones: valor predeterminado, 64 y 768 MiB, dos pasadas cada uno. Cero pérdidas y mismos píxeles en el fotograma comparado. Medianas de memoria privada máxima: 319,75 / 266,31 / 515,21 MiB; CPU de un núcleo lógico: 66,38 / 73,09 / 65,11 %. Se conserva el valor predeterminado: reducir memoria aumentó CPU y ampliar la caché no ofreció una ventaja suficiente.
- La sincronización por audio, velocidad 1 y ausencia de interpolación se conservan. No se fuerza una nueva política de sincronización a partir de una prueba corta. Para 24 fps en 60 Hz, la cadencia desigual de refrescos no demuestra por sí misma un defecto del reproductor. El ajuste opcional de frecuencia en pantalla completa sigue disponible cuando el visualizador admite el modo o un múltiplo exacto.

## Integridad y límites

Las configuraciones e historiales personales de ambos reproductores se restauran tras las comparativas y se verifican. Los registros privados de configuración no forman parte del paquete público. No se modifican los vídeos originales.

Las muestras son sintéticas o de referencia, en un único equipo Windows 11. No se certifica la ausencia de bugs, calidad colorimétrica física, todas las GPU, todos los ISO MVC/UHD, HDMI bitstream, Quest/Virtual Desktop ni compatibilidad universal. Las pruebas de fluidez no sustituyen una validación larga con cada archivo y visualizador real.

Entrega portátil sin instalador; no se publica en GitHub sin autorización para esta versión. Las fuentes correspondientes del motor se conservan con revisiones fijadas y sin cambios de bibliotecas.

## Ampliación de persistencia, sin cambio de versión

Posición vertical de subtítulos, brillo izquierdo/derecho y compensación de crosstalk se guardan automáticamente en `config/playback.json`, junto al aspecto de subtítulos. Se escriben solo al confirmar cambios, nunca por fotograma. La sección opcional `visual` recupera únicamente esos cuatro valores; no altera la selección de formato 3D. Los archivos antiguos sin esta sección conservan sus valores previos/predeterminados. Los valores inválidos se descartan. Restablecer aspecto o calibración guarda también el restablecimiento. Los perfiles manuales siguen disponibles y no se sobrescriben.

Pruebas específicas: restauración entre instancias, compatibilidad con preferencias antiguas, límites inválidos, conservación de tamaño/color al guardar posición, persistencia inmediata de cada control, restablecimiento y reversión del motor ante fallo de escritura. Los cambios siguen usando escritura atómica y los sondeos `--probe` no guardan preferencias personales. No se modifica el motor de vídeo ni las bibliotecas.

## Corrección de navegación de los editores persistentes

Se conserva la versión 1.0.2. Los paneles de tamaño/color/posición ahora mantienen una página padre «Aspecto de subtítulos»; los de brillo y crosstalk, una página padre «Ajuste de brillo por ojo». Al entrar se muestra directamente el parámetro solicitado, con «Volver» visible y pulsable en la cabecera. También se puede retroceder con flecha izquierda. Desde la página padre se accede a los demás parámetros y al restablecimiento correspondiente, sin cerrar/recrear el cliente de entrada del panel.

Los cambios y el guardado automático se mantienen al retroceder. Los valores marcados se actualizan en la jerarquía para no recuperar las copias iniciales al reabrir un submenú. Escape, clic fuera y el cierre habitual siguen disponibles. Tamaño/color continúan limitados a SRT; no se modifica el tratamiento de ASS/PGS ni las bibliotecas.

Cinco pruebas focalizadas aprobadas: marcado tras retorno/reentrada, navegación de brillo/crosstalk, navegación de aspecto de subtítulos, regresión de posición y guardado/reversión ante fallos de escritura. Se prueban retorno con clic y teclado, paso entre parámetros, selección posterior, restablecimiento y cierre en las cinco proyecciones. No se certifica todo dispositivo físico.

## Paneles compactos de ajuste, versión conservada 1.0.2

Esta revisión sustituye las listas anidadas de los editores anteriores por dos paneles de parámetros visibles. Subtítulos muestra tamaño y posición con valor, −/+/↺, nombre y muestra del color actual, seis colores pulsables y restablecimiento general. Brillo muestra izquierdo/derecho/crosstalk con porcentaje, −/+/↺ y restablecimiento general. En los menús de acceso también se muestran los valores vigentes. La ubicación sigue usando la columna y anclaje anteriores; únicamente se compacta el contenido del panel. No se cambian los menús contextuales generales ni se centra el panel.

Los límites desactivan el botón que no puede avanzar. Se conservan los pasos existentes: tamaño con los ocho porcentajes admitidos, posición en pasos de 5 puntos, brillo en pasos de 5 puntos y crosstalk de 1 punto. ↺ restablece 100 % de tamaño/posición/brillo y 0 % de crosstalk. Para color, Blanco recupera el neutro; Restablecer subtítulos recupera también el resto del aspecto. Aplicación y guardado atómico por pulsación; no hay temporizador nuevo, arrastre continuo ni reconstrucción del vídeo al pasar el ratón. Los botones usan las coordenadas lógicas del mismo compositor y se proyectan junto al panel por ojo. Se conserva el cierre y la protección ante cambio de archivo o pista.

La validación gráfica automatizada cubre selección de parámetros y colores, restablecimiento por fila y general, continuidad de pistas/filtros en subtítulos y las cinco proyecciones. Se inspeccionan capturas reales de ambos paneles. La batería completa de aplicación y el portátil extraído se verifican antes de entregar. Las pruebas son locales; no certifican todo visualizador físico ni la desaparición de fallos intermitentes históricos. El primer sondeo de calibración encontró un fallo del controlador de pruebas al recorrer un botón desactivado (null); se corrigió la iteración del controlador y se conservó el registro inicial.

### Resultado de la batería de esta revisión

La primera pasada serial completa terminó con 59 pruebas aprobadas, dos fallidas y 13 optativas excluidas (`panel-app-all.log`). Ambas fallidas dependían de la antigua jerarquía de porcentajes. Al adaptar la prueba de posición se detectó además que los nuevos IDs 7140–7142 no se despachaban desde Subs: se corrigió ese acceso y la disponibilidad de Color para SRT. Los dos casos se repitieron y aprobaron; se repitieron también el panel de subtítulos y la prueba de guardado/reversión, todos aprobados (`panel-corrected-*.log`). No se presenta la pasada inicial como una batería sin fallos: hay 61 pruebas de aplicación distintas aprobadas entre la pasada y las repeticiones focalizadas. No se repitieron las 13 pruebas optativas ni se atribuyen a esta revisión mediciones físicas nuevas de hardware.

Se comprueba formato, Clippy, compilación release, manifiesto/hash del portátil y arranque extraído con PATH limitado a Windows. Las 125 DLL y los ocho archivos originales de configuración/historial se verifican sin cambios. No se genera instalador ni se publica en GitHub.

## Corrección de pérdida de sesión al cambiar de ojo en MVC

Versión mantenida: 1.0.2. ISO autorizada probada: `3 A.M.2D+3D 2012 Blu-Ray 1080p MVC TrueHD 5.1-TTG.iso`. Antes de corregir, seleccionar Izquierdo pasaba de `cine-mvc://1` a `bd://mpls/00001`, incrementaba la generación y cargaba de nuevo el vídeo. La prueba corta recuperó la posición; confirmó la sustitución del flujo, no una pantalla negra permanente. Registro original: `eye-mvc-before.log`.

Se conserva el transporte MVC ya activo al cambiar de ojo o modo de presentación. La retención se limita al mismo origen/título: no se impone al abrir otra película o seleccionar otro título. Una apertura nueva en 2D conserva la política de usar AVC base sin trabajadores MVC; pasar a Derecho desde esa ruta inicia MVC cuando hace falta. La continuidad tiene un coste explícito: tras pasar de MVC a 2D siguen decodificándose ambas vistas, en lugar de reabrir el disco para ahorrar CPU. No se degrada la imagen para evitar esa reapertura.

Verificación focalizada: ocho cambios de ojo/modo en la ISO real, reproduciendo y en pausa, sin cambio de URI/generación, pistas ni estado de pausa y con avance posterior del vídeo. Apertura limpia en Izquierdo: AVC base 1920 con D3D11VA en este equipo. Paso asíncrono de base AVC a Derecho MVC, retorno a Izquierdo sin recarga y apertura posterior de MKV sin arrastrar MVC. Muestras de Full/Half SBS, OU/HOU, ASS 3D SBS/HOU, SDR y UHD HDR10: 88 cambios en reproducción/pausa, sin sustituir fuente ni perder pistas/posición. Siete regresiones de fiabilidad y una prueba de seek/pistas/stop/reapertura/cierre aprobadas. Los registros están en `work/mejoras-2026-11-03/eye-*.log`.

El primer sondeo multiformato exigía avance dentro de 120 ms y falló en ASS HOU. Se reemplazó esa exigencia por la espera explícita de avance, con límite de tres segundos y comprobación de fuente/pistas/posición. En la pasada final se observaron hasta 0,750 segundos para detectar avance tras alguna transición ASS; no hubo pérdida sostenida de sesión. No se afirma que toda transición sea instantánea ni que esta prueba certifique todos los ISO/hardware. El error histórico de metadatos gamma continúa con causa no demostrada. No se modifican las bibliotecas, los menús ni sus ubicaciones.

La prueba optativa de cancelación inicialmente agotó su límite porque esperaba que «Reproducir en 2D» eliminase el flujo MVC, condición que contradice la nueva continuidad. Se actualizó para comprobar que elegir un ojo conserva la generación y se usa después el cambio explícito de formato de entrada a 2D para provocar la liberación. Se verificaron cinco ciclos de preparación/cancelación y cierre en menos de cinco segundos: prueba aprobada en 2,35 s. Se conserva el registro inicial y el de repetición (`eye-iso-cancel*.log`). Total focalizado: once pruebas aprobadas, incluidas la ISO real, ocho tipos de muestra, siete regresiones de fiabilidad y la prueba de transporte asíncrono.

## Accesos únicos a los paneles, sin cambio de versión

Se eliminan los submenús redundantes de tamaño/color/posición y de brillo izquierdo/derecho/crosstalk. «Pista de subtítulos → Aspecto de subtítulos…» y «Vídeo 3D → Avanzado → Ajuste de brillo por ojo y crosstalk…» son ahora acciones directas únicas. Los botones de restablecimiento quedan dentro del panel correspondiente. Se cambia también el título del panel de calibración para incluir crosstalk. Versión conservada: 1.0.2.

No se modifica la geometría, el anclaje, el motor, las bibliotecas ni la lógica de aplicación/guardado. Se retienen los IDs internos necesarios para controles y compatibilidad, sin mostrarlos como entradas repetidas.

Cinco pruebas focalizadas aprobadas: accesibilidad coherente desde menú contextual y controles inferiores (con comprobación de entradas únicas y ausencia de hijos), 65 posiciones desde el acceso directo en cinco proyecciones, panel de subtítulos, panel de calibración y restablecimiento de aspecto para SRT/ASS/PGS. Formato, Clippy y compilación release verificados. EXE, ZIP portátil y fuentes actualizados; prueba de arranque extraído con PATH limitado a Windows. Los registros de esta revisión usan el prefijo `single-entry-` dentro de `work/mejoras-2026-11-03`.

## Posición ASS desactivada y revisión de duplicaciones, versión 1.0.2

La fila Posición queda deshabilitada para pistas ASS/SSA: sin porcentaje ni explicación adicional y sin acciones en sus tres botones. La comprobación de codec protege también la ejecución de comandos internos, evitando modificar o guardar una posición inefectiva para ASS. No se cambia el renderizado ni se fuerzan sus estilos/coordenadas. SRT y PGS conservan su posición; la selección de otra pista actualiza la disponibilidad. El restablecimiento general mantiene su alcance sobre las preferencias globales.

Se conserva una única entrada para cada panel. En Color se mantienen solo nombres y marca de selección; se eliminan muestras y la reiteración del nombre/color elegido en la cabecera. No se cambia la ubicación ni la versión.

Seis pruebas focalizadas aprobadas, y ampliación posterior de la prueba por píxeles también aprobada: 130 selecciones de posición (13 valores × cinco proyecciones × SRT/PGS); ASS deshabilitado en ocho rutas de salida; movimiento visible y restablecimiento de SRT/PGS en esas ocho rutas, con pausa/reproducción y seeks; ocho tamaños y seis colores SRT aplicados y comprobados por parámetros/píxeles; selección única de color; reactivación al cambiar de ASS a SRT/PGS; guardado y recuperación en una nueva instancia de State; conservación de estilos ASS y bitmaps PGS; accesos únicos desde menús. Las rutas incluyen 2D, Full/Half SBS, Full/Half OU, anaglifo, SBS anamórfico y composición Frame Packing. Los resultados son locales y no certifican todos los archivos/dispositivos posibles.

Registros: `ass-disabled-*.log/json` en `work/mejoras-2026-11-03`. Formato, Clippy, compilación release, paquetes y arranque portátil se verifican antes de entregar. Se conservan configuraciones originales y las 125 DLL del motor. No se genera instalador ni se publica en GitHub.

## Posición ASS oculta y retorno real al menú anterior

Se corrige la incoherencia entre desactivar y ocultar: la fila de posición ya no forma parte del panel cuando la pista es ASS/SSA. La protección interna se conserva y SRT/PGS mantienen la fila. No se agrega una explicación a la interfaz.

Los paneles unificados no tenían una página padre en su pila; por eso el encabezado solo mostraba retroceso en otros menús anidados. Se incorpora una acción de retorno propia del panel: «Volver» o Retroceso/Alt+Izquierda cierran el editor y devuelven al selector de pistas de subtítulos o al menú Avanzado. Se reutiliza la presentación nativa/estéreo existente para el padre, con título correspondiente; la geometría y anclaje del panel no cambian. Los bucles de navegación permiten reabrir sin apilar llamadas por cada retorno. Escape/cruz/clic fuera continúan cerrando. Los controles −/+ y el guardado no cambian.

Seis pruebas focalizadas aprobadas (`real-back-*.log/json`): retorno con botón y teclado, reapertura y continuación del ajuste en cinco proyecciones para ambos paneles; ausencia de posición ASS; regresión por píxeles de SRT/PGS, incluidos tamaños/colores/posiciones y restablecimiento; 130 selecciones de posición SRT/PGS; navegación de menús estéreo desde ambos ojos, cierre por segunda pulsación y continuidad de audio; coherencia de accesos sin entradas duplicadas. Se mantienen los límites de validación local descritos en el informe.

Versión conservada: 1.0.2. Formato, Clippy, compilación release, fuentes, hashes y arranque del portátil extraído se verifican antes de entregar. Sin cambios en las bibliotecas ni publicación automática.
