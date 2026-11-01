# Enigmax CinePlayer 0.2.23 — Windows 11 x64

Cambios y límites de la versión vigente: **[validación de esta entrega](#validación-de-esta-entrega)**. Se corrigen el cursor estéreo y la conservación de la ventana, se preservan los estilos ASS y se simplifican los menús. Versión de desarrollo; no se certifican todas las pantallas o receptores.

## Reproducción y subtítulos: novedades 0.2.23

- **Opciones → Reproducción:** lectura adaptativa, frecuencia automática opcional, cursor estéreo y desentrelazado automático opcional.
- **Subs → Aspecto de subtítulos:** tamaño entre 50 y 250 %, seis colores y restablecimiento para SRT. ASS/SSA y PGS conservan tamaño, color y posiciones originales; la profundidad 3D sigue disponible.
- Los ajustes se guardan atómicamente en `config/playback.json`. Los perfiles explícitamente guardados en `config/screens.json` recuperan salida SDR/automática, dispositivo de audio disponible, PCM/bitstream al cambiar de pantalla. No son una calibración colorimétrica automática ni perfiles ICC generados por el reproductor.
- La lectura adaptativa usa datos comprimidos: reserva orientativa de tres segundos, ampliable a ocho ante espera de caché; máximo 128 MiB, o 32 MiB en mínimo consumo. No aumenta la cola de vídeo sin comprimir MVC. No garantiza resolver saturación o averías del disco. Los saltos MVC dentro del par actual y los cortos hacia delante reutilizan el decodificador, sin otra caché de imágenes.
- La frecuencia automática está desactivada por defecto. Solo actúa en pantalla completa y conserva resolución, profundidad y orientación. Se ejecuta en otro hilo, comprueba modos del controlador e intenta restaurar el anterior al terminar, salir de pantalla completa o cerrar normalmente. No escribe modos personalizados ni cambia el registro. La API DEVMODE no distingue de forma fiable todas las frecuencias fraccionarias: comprueba los Hz reales en Diagnóstico. No activa HDMI 3D. Un cierre forzado o un fallo del controlador puede impedir restaurar el modo.
- El cursor integrado se proyecta por ojo en SBS/OU y Frame Packing sobre los controles y menús. Anaglifo y 2D usan el cursor de Windows. La integración física con Virtual Desktop/Quest requiere validación adicional.
- **Audio → Filtros:** Original, Modo nocturno y Claridad de voz. Los efectos son opcionales y se suspenden con bitstream para conservar el flujo original. Se han retirado los ajustes manuales de sincronización de audio y subtítulos, incluidos sus atajos.
- El diagnóstico muestra caché, búsqueda, descartes, A/V y formatos de entrada/salida. Los contadores acumulados no demuestran una causa física. No se añaden miniaturas, análisis visual, servicios residentes ni inferencia IA.

HDR 2D utiliza libplacebo y ST2094-40, con metadatos dinámicos disponibles y alternativa sin ellos. El renderizador compatible usa BT.2390. SDR conserva su tratamiento de origen; no se activa conversión inversa a HDR. La salida real depende de Windows y de la pantalla: no existe una calibración universal para TV, monitores, proyectores y Quest. Véanse las pruebas y excepciones en el informe.

Versión de desarrollo funcional. Se mantiene únicamente la última versión del programa; las versiones obsoletas se eliminan por indicación del usuario. Logo A elegido: cinta dorada y símbolo de reproducción azul.

Corrección MVC incorporada desde 0.2.3: **VERIFICACION_0.2.3.md**. Se reconstruyen las unidades NAL fragmentadas entre paquetes PES del SSIF para evitar corrupción del ojo derecho, comprobada con TALKTOME_3D.iso. Las mejoras de interfaz y la batería previa de sincronización están documentadas en **VERIFICACION_0.2.2.md**.

## Abrir y reproducir

Extrae el ZIP completo y ejecuta `EnigmaxCinePlayer.exe`. Conserva `libmpv-2.dll` y la carpeta `runtime/`. No necesitas instalar Rust ni FFmpeg para utilizarlo. Arrastra un MKV, MP4 o ISO, o pulsa **Ctrl+O**. Para abrir por doble clic, elige el ejecutable mediante «Abrir con» de Windows. Las ISO se leen directamente; no se montan ni se modifican.

La versión aparece en el título, en **Ayuda / F1** y en **Acerca de** dentro del menú general. Se obtiene de Cargo.toml al compilar.

**M / clic derecho:** menú. **Ctrl+3:** vídeo 3D. **Espacio:** pausa. **F / doble clic:** pantalla completa. **Esc:** ventana. **1:** tamaño original, sujeto a los límites de pantalla. **Flechas:** buscar. **Ctrl+S:** subtítulos externos. **Q:** cerrar.

## Presentación estereoscópica

Al abrir cada vídeo reconocido como 3D se pausa temporalmente y se pregunta cómo verlo: **2D sin gafas**, **anaglifo** con las gafas elegidas o **dos vistas originales** para una pantalla 3D. Se pregunta de nuevo al abrir otro archivo o reabrir el mismo; un cambio interno de transporte MVC conserva la elección. Al cerrar la pregunta sin elegir se utiliza 2D izquierdo.

**Vídeo 3D / Ctrl+3** reúne las opciones habituales: **Ver en 2D**, **Ver con gafas de colores**, **Mostrar las dos vistas originales** e **Invertir profundidad**. Elegir las gafas activa directamente el anaglifo. Hay cinco parejas: rojo/cian, verde/magenta, ámbar/azul, rojo/azul y rojo/verde; estas dos últimas son monocromas. **Formato 3D** corrige la detección. **Avanzado** agrupa el ojo 2D, método de color, calibración y ASS 3D. **Calidad / consumo** está en el menú principal, bajo **Color / HDR**.

El modo automático reconoce etiquetas explícitas del nombre como Full-SBS, HSBS, Full-OU, HOU y MVC, metadatos inequívocos y títulos Blu-ray con vista dependiente. Un nombre genérico «SBS», «3D» o unos metadatos sin información suficiente sobre Full/Half abren primero la confirmación del formato. No se deduce Full/Half por la resolución ni se analiza toda la imagen; un archivo sin etiquetas ni metadatos puede requerir selección manual. Las preferencias de formato y presentación de la película anterior se reinician para evitar aplicarlas al siguiente vídeo 2D. El menú permite corregir el aspecto final de fuentes anamórficas o recortadas.

Los controles inferiores y los menús propios permanecen en el plano de la pantalla, sin desplazamiento de profundidad. En SBS/OU y Frame Packing se proyectan en las mismas coordenadas de cada ojo; en anaglifo no hay separación horizontal entre los canales de color. La misma transformación se aplica a las pulsaciones, al progreso y al volumen. La barra aparece al mover el ratón y se oculta tras la inactividad. Un clic fuera de la barra pausa o reanuda, con una espera breve para distinguir el doble clic, que mantiene pantalla completa. Los botones y el deslizador conservan sus propias acciones. La barra se actualiza como máximo unas diez veces por segundo mientras está visible; la búsqueda con el deslizador se ejecuta al soltarlo.

La composición GPU separa los ojos antes del escalado, trabaja en RGB lineal y no recodifica el archivo. La calibración permite ajustar las ganancias de cada ojo y compensación de fugas hasta 15 %. Los antiguos ajustes de convergencia del vídeo y profundidad añadida a los subtítulos se han retirado; los perfiles anteriores se admiten, pero esos valores se ignoran.

**Calidad / consumo:** Equilibrado usa spline36; Máxima calidad, EWA Lanczos; Mínimo consumo, bilineal y menor caché, sin cálculo del pico HDR por píxeles. Se conserva resolución, frecuencia y audio. Compatibilidad utiliza CPU/RGB de 8 bits únicamente para anaglifo; con subtítulos fusionados o calibración se conserva la GPU para mantenerlos. No es un modo de ahorro. La selección se guarda automáticamente en `config/quality.json`, independientemente del resto de ajustes, y se recupera al arrancar. Si no puede guardarse se informa del error y se restablece la selección anterior. La carpeta del programa debe permitir escritura para conservar ajustes. Consulta las mediciones y diferencias de calidad del informe vigente.

Al iniciar sin un archivo indicado, se carga el último vídeo en pausa, con su playlist, posición y pistas guardadas. Bajo el título aparecen **Continuar reproducción** y **Empezar de nuevo**. La reproducción espera esa elección; Espacio o Intro eligen continuar. Si no existe un último archivo disponible, se muestra **Arrastra tu vídeo aquí**. Ctrl+O y arrastrar archivos siguen disponibles. La posición pendiente se conserva durante las recargas MVC.

## Blu-ray e ISO

El menú **Títulos Blu-ray** muestra playlist, duración, número de clips, presencia de SSIF y repeticiones. La selección inicial prioriza un título 3D sin segmentos repetidos; en discos 2D escoge el más largo sin repeticiones. Es una heurística: comprueba el título y elige otro cuando corresponda. No se implementan menús BD-J ni descifrado AACS/BD+.

Para anaglifo o vista dependiente se utiliza un decodificador MVC real. Solo se presenta como MVC cuando se han recuperado ambos ojos. La lectura conserva audio y subtítulos del título seleccionado. El transporte usa una cola pequeña de vídeo sin comprimir; no crea una conversión temporal de la película. Para mostrar la vista base en 2D se evita decodificar la segunda vista y se mantiene la ruta nativa de menor consumo.

El soporte MVC está en desarrollo: las pruebas reales se han realizado con la ISO Hyperspace facilitada. MKV/MP4 MVC, playlists con múltiples clips y cambios de formato necesitan más muestras reales. El transporte actual exige frecuencia constante, dimensiones constantes y YUV 4:2:0 de 8/10 bits. Rechaza discontinuidades que impidan conservar la sincronización. No convierte contenido 2D en 3D.

## HDR, sonido y subtítulos

SDR y HDR10 usan gpu-next/D3D11. El modo automático depende del estado HDR de Windows y de la pantalla. También puedes forzar SDR. La ISO UHD facilitada se ha abierto como HEVC 3840×2160 PQ/BT.2020 con D3D11VA y audio TrueHD. Eso no certifica la señal enviada al televisor.

No se garantiza passthrough de metadatos HDR10+ ni salida nativa Dolby Vision, ni reconstrucción FEL verificada físicamente. La interpretación de RPU/reshaping y la reproducción de una capa HDR compatible no equivalen a activar Dolby Vision en la TV. No se ha verificado físicamente una cadena HDR/HDMI.

WASAPI/PCM para altavoces o TV; selección de dispositivo y pistas en el menú. Passthrough opcional AC3/DTS o HDMI ampliado AC3/EAC3/DTS-HD/TrueHD, según receptor y controlador. No se ha comprobado con un receptor físico ni mediante escucha.

SRT y ASS/SSA conservan el renderizado nativo, incluidos estilos y fuentes adjuntas. **Nuevo: los ASS con eventos estereoscópicos duplicados se detectan y se procesan junto con el vídeo.** Consulta el apartado siguiente. Los subtítulos convencionales se duplican por ojo en las salidas estereoscópicas, sin profundidad añadida. Las pistas PGS del Blu-ray se enumeran y seleccionan con su decodificador nativo; la validación visual de PGS sigue pendiente y no se detectan ni transforman PGS estereoscópicos duplicados. Los controles de posición no sustituyen las posiciones explícitas de un ASS.

## Subtítulos ASS 3D

Activado por defecto: **M → Vídeo 3D → Avanzado → Subtítulos 3D → Automático**. Se comparan eventos simultáneos de la pista ASS seleccionada: mismo texto, tiempos, capa y estilo, con posiciones correspondientes en mitades opuestas. Se reconocen posiciones explícitas, movimientos sencillos y márgenes de estilos equivalentes. La identificación queda asociada a esa pista y se reinicia al cambiar de archivo o de subtítulo. No se mezclan dos pistas independientes ni se interpreta como 3D una pista solo por su nombre.

Cuando se confirma una pareja SBS u OU compatible con el formato de vídeo seleccionado:

- Anaglifo: texto y vídeo pasan por la misma matriz de gafas, intercambio de ojos y calibración.
- 2D izquierdo o derecho: se selecciona el mismo ojo para imagen y texto.
- Original: se conservan ambas vistas y su disposición.

Se mantiene la disparidad ya escrita en el ASS; no se inventa una profundidad nueva. La corrección de aspecto utiliza PlayResX/PlayResY: admite, por ejemplo, un ASS Full SBS 3840×1080 sobre vídeo HSBS 1920×1080. No reescribe líneas, estilos, fuentes ni tiempos. El archivo original permanece intacto.

El renderizador fusiona el ASS nativo con el cuadro antes del shader. Para esa pista se utiliza temporalmente la ruta `gpu` de mpv con D3D11; se vuelve a `gpu-next` al elegir un subtítulo 2D o desactivar la función. Se mantiene la decodificación hardware cuando el sistema la admite. La composición anaglifa CPU de compatibilidad se sustituye por GPU mientras haya ASS 3D activo, para que vídeo y texto reciban exactamente la misma transformación; el programa lo indica. En una ISO MVC mostrada por su vista base, un ASS 3D puede requerir decodificar ambas vistas para componerlos juntos.

La detección es conservadora y empieza al aparecer una pareja reconocible, sin recorrer toda la película ni extraer su pista. Para scripts complejos o falsos positivos están **Forzar ASS 3D SBS**, **Forzar ASS 3D OU** y **Subtítulos 2D · no transformar**. Requiere un lienzo ASS explícito válido y un formato de vídeo concordante. Las transformaciones de posición anidadas, pares con contenido/tiempos distintos, algunos recortes complejos y autorías inusuales pueden requerir selección manual. No se garantiza identificar cualquier ASS por análisis semántico. Los subtítulos secundarios simultáneos no forman parte de esta validación.

Pruebas específicas y límites: **VERIFICACION_ASS3D.md**. `verificar-ass3d.ps1` ejecuta 24 casos sintéticos; `verificar.ps1` mantiene la regresión anterior. `build.ps1` también prueba cambios ASS 3D/SRT, búsqueda y conservación de un ASS externo con libmpv real.

## Perfiles y recursos

Puedes guardar opciones como predeterminadas o guardar/cargar manualmente un perfil por archivo. Se almacenan junto a la aplicación en `config/`, con validación de límites y sustitución atómica. El shader temporal de cada proceso se elimina al cerrar. La carpeta portátil debe permitir escritura.

El bucle Rust espera eventos sin sondeo continuo. Búfer nativo de paquetes: 64 MiB delante y 8 MiB detrás. En transporte MVC: 16 MiB delante, sin búfer trasero y 0,25 segundos de anticipación; una reserva de cuadro reutilizada. Estos valores no limitan la memoria total del proceso. Decodificador, referencias de vídeo, GPU, audio y subtítulos necesitan memoria adicional.

En una prueba MVC de 0.2.0 de 20 segundos sobre i9-9900K se observaron 745,7 MiB de memoria privada máxima y cero descartes; no es un máximo garantizado ni una prueba prolongada. El ejecutable usa x86-64 básico y el decodificador selecciona SIMD en ejecución. La compatibilidad y fluidez con todos los Intel desde 2019 no están certificadas.

## Compilación y arquitectura

`build.ps1` compila el puente nativo, comprueba formato, ejecuta clippy con advertencias como errores, pruebas Rust y release. Requiere Rust GNU y MinGW/make/windres; en este equipo están aislados en `work/tools`. Incluye fuentes de edge264-mvc y libudfread y SDK/import libraries del FFmpeg compartido utilizado. Las DLL FFmpeg de `runtime/` se conservan al recompilar. Consulta DEPENDENCIAS.md y licencias/.

`src/lib.rs` y `src/stereo.rs` no dependen de Windows. `player.rs` contiene las políticas de reproducción; `mpv.rs` y `mvc.rs`, enlaces ABI; `windows.rs`, interfaz nativa. El puente C actual depende de Windows. GOOGLETV.md describe el trabajo pendiente para Android: no se entrega APK.

VERIFICACION_ASS3D.md documenta esta ampliación; VERIFICACION.md conserva las pruebas de 0.2.0. No se ha certificado compatibilidad universal. Las fuentes correspondientes del motor y sus dependencias se ofrecen en la misma release: FUENTES.md. Se distribuye como ZIP portátil y se actualiza mediante GitHub Releases; los ejecutables no están firmados digitalmente.

## Actualizaciones desde GitHub

Al abrirse y desde Acerca de > Buscar actualizaciones, consulta la última release estable en un hilo secundario. Solo descarga si aceptas. La actualización utiliza el ZIP portátil completo, verifica hashes, rutas, versión y arquitectura, espera al cierre y sustituye EXE y bibliotecas conservando config/. No ejecuta instaladores ni modifica asociaciones. Ante una sustitución fallida intenta recuperar los archivos anteriores. Consulta ACTUALIZACIONES.md para requisitos, límites y publicación.

`build-portable.ps1` genera un ZIP con los archivos necesarios para ejecutar la aplicación y sus licencias; excluye código, herramientas, pruebas y ajustes personales. El código se mantiene por separado en el repositorio. La preparación de actualizaciones usa almacenamiento temporal, pero las bibliotecas de reproducción permanecen en su carpeta y no se extraen a TEMP para ejecutarlas.

## Cambios de 0.2.7

Distribución y actualización del conjunto portátil, recuperación ante archivos bloqueados y validación estricta del ZIP. La ventana de descarga atiende eventos durante la conexión y lectura de red. Se descarta una posición de reanudación pendiente al abrir otro archivo; los errores de presentación y ASS se registran sin diálogos que bloqueen el consumidor de eventos. La aplicación puede continuar si falla la asignación del icono. Auditoría y límites: VERIFICACION_0.2.7.md.

## Cambios de 0.2.5

En el menú Acerca de se puede buscar actualizaciones manualmente; la consulta sigue ejecutándose en segundo plano e informa del resultado. La barra inferior incluye Audio y Subs (A/S en ventanas pequeñas), con las pistas disponibles y desactivación de subtítulos. Las búsquedas, cambios de pista y controles de reproducción se envían de forma asíncrona. Las búsquedas rápidas se agrupan con una solicitud activa y una pendiente; se descartan al cambiar de archivo o cerrar. Los errores de controles se muestran sin diálogo modal y se conserva un diagnóstico acotado en config/ultimo-error-reproduccion.log. Consulta VERIFICACION_0.2.5.md para el alcance de las pruebas.

## Cambios de 0.2.8

Motor reconstruido desde revisiones fijadas de mpv y FFmpeg; ambos comparten las bibliotecas FFmpeg. Carga de dependencias desde runtime/ mediante directorios DLL explícitos, sin buscar en PATH o el directorio de trabajo. Inventario, licencias y paquetes de fuentes correspondientes incluidos. Verificación: VERIFICACION_0.2.8.md.

## Cambios de 0.2.9

El selector inicial de vídeo 3D usa botones grandes accesibles de Windows. Ofrece 2D izquierdo, anaglifo con cinco gafas, Full SBS 3840×1080 y Full SBS anamórfico 3840×2160. Los dos últimos componen dos vistas de 1920×1080 o 1920×2160 en la GPU: expanden HSBS y convierten OU/MVC a SBS. El modo anamórfico estira la altura y se verá deformado antes de activar SBS en la pantalla 3D. No crea ni convierte archivos en disco ni cambia la resolución HDMI de Windows. En un monitor de otra resolución, la composición se adapta al área de reproducción.

Cada shader tiene una ruta inmutable por contenido durante la sesión. Así se evita que la caché de mpv mantenga las gafas, la inversión o el shader anaglifo anterior al elegir 2D. Las variantes iguales reutilizan el mismo archivo y la carpeta de sesión se limpia al cerrar.

La aplicación recuerda la posición en config/resume.json y pregunta al reabrir el mismo medio si se quiere reanudar o empezar desde cero. Incluye el título MPLS de las ISO. Solo conserva posiciones a partir de 10 segundos y a más de 10 segundos del final; un vídeo terminado deja de ofrecer reanudación. La identidad combina ruta, tamaño y fecha de modificación para evitar aplicar posiciones a otro archivo sustituido. Máximo 500 registros y un límite de tamaño; escritura atómica. Se actualiza en memoria cada 5 segundos y en disco cada 30 segundos, además del cierre y cambio de archivo. Un cierre forzado puede perder hasta el último intervalo de guardado; el cierre nativo puede recuperar la última muestra de 5 segundos si mpv ya ha finalizado.

Verificación y límites: VERIFICACION_0.2.9.md.

## Cambios de 0.2.10

El EXE declara PerMonitorV2 desde su manifiesto, antes de crear ventanas. La pantalla completa utiliza el área completa del monitor que gestiona mpv, sin modo exclusivo ni cambios de resolución de Windows. Se comprobó a 3840×2160 con escalado del 150 %, tanto con gpu-next como con gpu. Las barras que correspondan a la relación de aspecto de la película se conservan.

La barra inferior añade «⋯», con el resto de opciones del menú contextual. Usa el mismo constructor y las mismas acciones, omitiendo las secciones de pistas y 3D, que tienen botones propios. Conserva salida de audio, dispositivo, aspecto, HDR, ayuda, información y actualizaciones.

Se explicita el fallback de mpv: métodos acelerados seguros, variantes con copia y, si no funcionan, decodificación software; tres errores de decodificación consecutivos disparan el siguiente método. El renderizador intenta gpu-next y después gpu, y Direct3D puede recurrir a WARP si no logra crear el dispositivo físico. Los hilos software se ajustan a los procesadores lógicos, con máximo 8 para FFmpeg y 4 para MVC. No se reducen la resolución ni la precisión del vídeo para ocultar una falta de rendimiento. Estos límites contienen la memoria por fotogramas y evitan cuatro trabajadores MVC en equipos con uno o dos procesadores lógicos; no garantizan menor consumo o más velocidad en cualquier equipo.

MVC conserva el decodificador software de ambas vistas. Los filtros CPU de compatibilidad y la conversión HDR para anaglifo conservan auto-copy: pueden decodificar por hardware, pero el procesamiento de esos filtros sigue en CPU. La composición normal y las salidas Full SBS usan GPU. Información de reproducción incluye el modo activo y el diagnóstico de ventana/monitor. Ver VERIFICACION_0.2.10.md para resultados y límites.

## Cambios de 0.2.11

El selector inicial prioriza el formato original HSBS/HOU, identificado por su nombre y con instrucciones para activar SBS o arriba/abajo en una pantalla compatible. Esta salida conserva el empaquetado y no crea las texturas intermedias de resolución fija. El escalado final del monitor sigue siendo necesario cuando su resolución difiere. 2D izquierdo y anaglifo permanecen disponibles; Full SBS 3840×1080 y SBS anamórfico 3840×2160 se presentan como alternativas de compatibilidad, con advertencias sobre interpolación y posible reducción de detalle de fuentes mayores. Para Full SBS/OU completos, la pantalla y su configuración de salida deben admitir ese empaquetado.

Las barras de volumen y progreso utilizan la misma proyección que el resto de controles. Se desactiva la barra 2D de mpv; rueda, teclas de volumen y búsquedas por teclado muestran el panel proyectado. El volumen también aparece en ventanas más pequeñas cuando queda espacio. El arrastre SBS conserva el ojo donde comenzó, no salta al cruzar la división y utiliza la coordenada final al soltar, incluso fuera del panel o cuando no llegó el último evento de movimiento. Shift+flechas izquierda/derecha permite búsquedas exactas de un segundo.

Una ISO puede conservar archivos AACS aunque su vídeo ya esté legible. Si libbluray rechaza abrirla, se intenta una única lectura UDF alternativa del mismo título, limitada a títulos 2D de un clip. Antes de admitirla se exige decodificar una imagen válida por software; después mpv usa su decodificación habitual, incluida la aceleración compatible. No se descifra contenido, no se modifica ni extrae la película y no se sustituye silenciosamente el título seleccionado. Esta vía conserva los flujos de audio y subtítulos del M2TS, admite búsquedas y limita el final al intervalo del título. No reconstruye capítulos MPLS ni menús; los títulos de varios clips siguen necesitando la vía Blu-ray habitual.

Ver resultados y límites en VERIFICACION_0.2.11.md. La versión no está publicada en GitHub y no se crea instalador.

## Cambios de 0.2.12

Vídeo 3D y la pregunta inicial incluyen Frame Packing 1920×2205 (composición). Produce en GPU dos vistas 1920×1080, una encima de otra, separadas por 45 líneas negras. Conserva la proporción de cada ojo mediante bandas cuando sea necesario, permite intercambiarlos y transforma también el ASS 3D fusionado. Es una composición de imagen, no una activación de Frame Packing HDMI: no cambia la resolución/frecuencia de Windows ni negocia el modo 3D del televisor. Si la ventana tiene otra proporción, el lienzo completo se ajusta a ella sin recortarse. HSBS/HOU originales siguen siendo las opciones recomendadas para pantallas compatibles con esos formatos.

Los controles se proyectan dentro de cada vista Frame Packing y excluyen el hueco y los márgenes laterales de las zonas de pulsación. También funciona el arrastre de progreso y volumen en cada ojo. El perfil conserva esta salida como delivery=3, sin cambiar los valores anteriores.

Al cambiar el volumen aparece durante aproximadamente 1,8 segundos un aviso con porcentaje y flecha de subida/bajada; silencio y reactivación tienen su propio mensaje. El aviso sigue el modo 2D, anaglifo, SBS, OU o Frame Packing. Reutiliza el panel y su temporizador, manteniendo el límite de unas diez actualizaciones por segundo y sin sondeo cuando está oculto. Se incorporan más datos de diagnóstico a petición en Información de reproducción: decodificador activo, renderizador, FPS y retrasos de presentación.

No se incorporan miniaturas, inferencia IA, decodificadores adicionales ni colas nuevas. Frame Packing utiliza un paso de composición GPU solo al seleccionarlo; puede requerir más memoria gráfica que el original sin composición. No se afirma consumo idéntico o mínimo universal. Pruebas y límites en VERIFICACION_0.2.12.md.

## Cambios de 0.2.16

Selector J con cinco tarjetas e iconos de acción directa, fotografías fijas embebidas y aspecto de cristal sobre una ilustración suavizada. Original y 2D no tienen descripción. Las gafas tienen selector visual y Volver. No se captura el escritorio ni la película, ni se añade un motor web o animaciones. Los recursos gráficos se liberan al cerrar la ventana.

Se retira Full SBS 3840×1080 y se migra su perfil a Original. Los modos conservan las muestras de origen; el anamórfico se ajusta a 16:9 al presentar, sin lienzo fijo 4K, y OU se reorganiza en SBS sin cambiar la cantidad de píxeles. Frame Packing conserva 1920×2205 por excepción expresa del usuario. La pantalla completa usa el visualizador, sin modificar sus temporizaciones.

Capítulo anterior, lista y capítulo siguiente están en la barra. En ventanas muy estrechas se mantiene la lista. Los títulos MVC importan capítulos de su fuente original. Ver VERIFICACION_0.2.17.md para pruebas, límites y coste de los modos.

## Interfaz M y auditoría de 0.2.16

Barra flotante con progreso verde suave, controles de capítulos/volumen y menús oscuros. Audio y Subtítulos muestran los datos de cada pista en dos líneas; Subtítulos incluye añadir pistas y ajustar la apariencia de SRT. Cerrar y Volver tienen mayor contraste. Se conserva el procesamiento del vídeo y la política de aceleración automática. Resultados y límites de audio, HDR, MVC y recursos: **VERIFICACION_0.2.17.md**.

Durante la reproducción, los menús de Audio, Subtítulos, 3D, Opciones, capítulos y sus submenús se componen dentro del vídeo en cada ojo. Una segunda pulsación de 3D cierra su desplegable; también admiten Escape, teclado y rueda. Se recuperan −10 y +10. La selección inicial mantiene las tarjetas visuales; durante la salida estéreo se emplean desplegables sobre la imagen para que ambas vistas reciban la interfaz. Los selectores de archivos de Windows y los diálogos del actualizador siguen siendo ventanas del sistema en 2D.

## Cambios de 0.2.17

Se eliminan los menús, el botón y los ajustes de convergencia del vídeo y profundidad añadida de subtítulos. Los perfiles antiguos ignoran esos dos valores, conservando el resto de preferencias y la disparidad escrita en los ASS 3D. El botón de formato se llama «Formato 3D». Controles y menús permanecen en el plano de pantalla. «Continuar último vídeo» recupera la ruta, playlist y posición de las ISO, conservando la búsqueda pendiente durante las recargas MVC. Pruebas y límites: VERIFICACION_0.2.17.md.

## Cambios de 0.2.18

Se recuerda el último vídeo al confirmar sus pistas, sin esperar necesariamente a la primera imagen. Las escrituras tardías de otras instancias no sustituyen una apertura más reciente y el historial se combina bajo bloqueo. Se guardan las pistas de audio y subtítulos por archivo, incluidos subtítulos desactivados y SRT/ASS externos. Las elecciones se restauran también al recargar el transporte. El actualizador usa un manifiesto público con SHA-256, conserva una caché de 12 horas para consultas automáticas y respeta las esperas del servidor también al consultar manualmente. Ver VERIFICACION_0.2.18.md.

## Cambios de 0.2.19

Calidad / consumo está en el menú principal, inmediatamente debajo de Color / HDR. Los tres perfiles existentes conservan sus índices y ajustes. Se añade Mínimo consumo · equipos modestos: escalado bilineal, análisis de pico HDR desactivado y límites de búfer de hasta 16 MiB delante y cero detrás (8 MiB delante en MVC). No cambia la resolución, los FPS, la salida 3D ni la selección de audio. Conserva la aceleración automática y el tone mapping. El escalado es más suave y el HDR depende más de los metadatos; el menor búfer trasero puede requerir volver a leer al retroceder. No equivale a un límite de memoria total ni garantiza fluidez en cualquier ordenador.

Al salir del perfil se restauran los límites reales anteriores del transporte. El botón 3D conserva su selector y ofrece Todas las opciones 3D; la etiqueta SBS se muestra como Reproducir en SBS Anamórfico. Audio y Subs muestran idioma y códec/formato de cada pista con nombres legibles; los datos ausentes no se inventan. El menú contextual y los cuatro accesos de la barra comparten sus constructores y acciones.

## Cambios de 0.2.20

La elección de calidad se guarda automáticamente. Los perfiles GPU cambian sin reconstruir innecesariamente el filtro; la alternativa CPU de anaglifo recibe las superficies aceleradas mediante una conversión explícita y conserva GPU cuando lo requieren subtítulos o calibración. Mediciones de recursos, transiciones y límites: VERIFICACION_0.2.20.md.

## Cambios de 0.2.21

La preparación pesada se ejecuta en un trabajador acotado y conserva el archivo actual ante fallos. El puente transmite las etiquetas principales de color y SAR; permite subtítulos incrustados aunque no haya audio. Los menús se invalidan al cambiar de archivo y el actualizador solicita el cierre normal. Pruebas y limitaciones: VERIFICACION_0.2.21.md.

## Validación de esta entrega

Fecha: 1 de noviembre de 2026. Base única vigente: este directorio, cuyo nombre histórico no indica la versión.

### Cambios

- Cursor estereoscópico mediante una franja bitmap transparente en el ID 63, por encima de los menús (59/60) y controles (61/62). Dos posiciones por ojo, sin lectura de vídeo ni textura del tamaño de toda la ventana. Coordenadas y tamaños acotados; movimiento comunicado con una propiedad observada que agrupa eventos. Los menús actualizan el cursor mientras su bucle ocupa el cliente principal. Se atiende movimiento también en las secciones inferiores de entrada y se limpia el cursor al cambiar de archivo.
- Cursor integrado solo para SBS, OU y Frame Packing. Anaglifo y 2D recuperan la política del cursor de Windows.
- Tamaño y color exclusivamente para pistas SRT. ASS/SSA conserva escala 1, estilos y posiciones originales, incluidos los pares 3D; se retira el ajuste que forzaba estilos ASS. PGS conserva su bitmap original. El procesamiento de profundidad estereoscópica existente no se elimina.
- Se elimina la segunda escritura de `sub-ass-override` desde la ruta de vídeo; la política de apariencia tiene un único responsable.
- Desactivado el redimensionado automático de la ventana al reconfigurar el vídeo. Se captura y restaura WINDOWPLACEMENT cuando hace falta cambiar entre gpu/gpu-next fuera de pantalla completa.
- Audio incluye Filtros: Original, Modo nocturno y Claridad de voz. Los filtros se suspenden con bitstream; no se recodifica el flujo para aplicarlos.
- Retirados diálogos y atajos de sincronización manual de audio/subtítulos. Los perfiles antiguos ya no restauran el retardo manual de audio. Se conserva el ajuste temporal interno del transporte MVC: no es un control manual.
- Reproducción contiene exclusivamente lectura adaptativa, frecuencia automática, cursor estéreo y desentrelazado.

### Validación ejecutada

- `build.ps1` vigente: tres pruebas C correctas, formato Rust, Clippy con advertencias como errores, 50 pruebas del núcleo y 50 del ejecutable aprobadas. Compilación release terminada. IO cancelable: 48 handles antes y después.
- Prueba GDI aislada de 100 ciclos de menú aprobada (0 → 1).
- Prueba adicional aislada de 10 ciclos de motor con dependencias residentes aprobada: tras el calentamiento, kernel/GDI/USER permaneció en 1135/12/2 hasta el último ciclo. En total, 102 pruebas Rust distintas aprobadas, contando las dos aisladas. Nueve pruebas opcionales permanecen sin ejecutar en esta entrega.
- Comparación de píxeles: SRT crece y cambia a amarillo; ASS no cambia al variar las preferencias de apariencia; PGS mantiene su bitmap. No demuestra que todo ASS imaginable sea correcto: por ese motivo se ha retirado la personalización de estilos ASS.
- Alternancia repetida SRT/ASS con cambio real de gpu-next/gpu: se mantienen posición y dimensiones de ventana. El cursor bitmap aparece por encima de los planos opacos de controles en la captura.
- Pruebas Lua de ciclo de menús y cursor: SBS completo/medio, OU completo/medio, Frame Packing y exclusión de anaglifo. Generación de 540 combinaciones de modo/dimensiones, con coordenadas válidas y fuera de ventana. Son pruebas controladas, no validación con gafas físicas.
- Estrategia adicional de recorrido de estados con orden pseudoaleatorio reproducible: 40 transiciones de pista, modo, pausa y seek, recorriendo Full SBS, HSBS, OU y HOU y cinco salidas. Se conservan geometría, pausa y primarias BT.709; no desborda la cola de eventos.
- Se repiten además las pruebas existentes de códecs de audio, cambio de pistas, profundidad de subtítulos, HDR→SDR, menús, cierre y errores de estructuras de disco. Decodificación contra sumidero nulo no certifica salida HDMI física ni escucha humana.

### Hallazgo adicional: descarga de dependencias

Al crear y destruir motores **descargando y volviendo a cargar sus DLL entre ciclos** se observó crecimiento sostenido de handles: aproximadamente 24 por ciclo con GPU y 21 con salida null, incluso con un segundo de espera. GDI/USER no crecieron. Manteniendo cargada la misma biblioteca, el crecimiento desapareció después del calentamiento.

La aplicación normal crea un único Mpv en `run()` y mantiene sus dependencias cargadas hasta salir. No se ha demostrado una fuga equivalente durante el cambio normal de película. La causa concreta dentro de las dependencias/runtime sigue sin identificarse; no se han añadido cierres manuales de handles ajenos ni cambios de arquitectura especulativos. La prueba opcional puede reproducir la recarga de DLL con `ENIGMAX_AUDIT_UNLOAD=1`; `ENIGMAX_AUDIT_VO=null` permite separar el renderizado GPU. Los registros de ambas variantes se conservan como evidencia.

### Validación final del EXE y entrega

- EXE restaurado por el usuario en `work/build/release`, versión de recursos 0.2.23, 2.840.064 bytes; SHA-256 `DE97308128720F76820FECCCC6AF363E30A6462DE3B5359F9BE7AB532BE64B21`.
- Muestra enviada al portal oficial de Kaspersky como privada y con informe dinámico completo. Resultado observado: **Clean**, cero detecciones y cero actividades sospechosas en ese análisis. No certifica por sí solo todas las dependencias ni toda posible ejecución. El usuario confirmó el envío de la petición a expertos sobre la contradicción con la alerta local; sin respuesta técnica confirmada todavía.
- El usuario autorizó continuar sin esperar esa respuesta. No se ha desactivado Kaspersky ni creado exclusiones.
- La copia de `outputs/EnigmaxCinePlayer-0.2.1/EnigmaxCinePlayer.exe` sigue bloqueada por otro proceso. La lectura y la sustitución devuelven archivo en uso. Se utiliza para pruebas y empaquetado el EXE restaurado que coincide exactamente con el hash analizado. La sustitución de la copia bloqueada queda pendiente de liberarla; esto no se presenta como una corrección del antivirus.
- `fault_probe.py`: siete casos aprobados. SRT, ASS y HDR reprodujeron y cerraron normalmente; MKV vacío/truncado, MP4 inválido e ISO inválida finalizaron con error controlado, en menos de un segundo cada caso dañado. Configuración personal intacta.
- `release_probe.py`: once casos aprobados con controles visibles: Full SBS, HSBS, OU, HOU, anaglifo, anamórfico, Frame Packing, PGS y tres aperturas/cierres de Hellcat. Capturas y estados en `verificacion/0.2.23/release/`.
- Se ejecutaron los dos tests optativos sobre Hellcat: 50 recargas con búsquedas y 30 cierres anticipados. Ambos aprobados. El total de pruebas Rust distintas ejecutadas y aprobadas pasa de 102 a **104**; tres pruebas C aprobadas. Siete pruebas opcionales siguen sin ejecutarse en esta entrega.
- Se revisaron visualmente las capturas SBS, HOU y anaglifo. No equivalen a una comprobación mediante visor o TV 3D físicos.
- El empaquetador permite indicar expresamente el EXE restaurado sin otra base de código. Comprueba la versión del recurso, hashes del payload y cada archivo del ZIP extraído en un directorio temporal propio; rechaza una versión equivocada antes de crear un paquete. El paquete contiene el informe vigente al que enlaza el README.
- La integridad y el estado de entrega se registran en `verificacion/0.2.23/delivery-status.json`. Las fuentes correspondientes se actualizan sin recomprimir innecesariamente las dependencias sin cambios.

### Límites

- Sin validación física de Virtual Desktop/Quest, receptores HDMI, HDR o todas las pantallas/DPI.
- Sin nueva campaña sobre ISO grandes: TALKTOME y Titanic no estaban disponibles en las rutas autorizadas. Hellcat sí se ha probado de nuevo; no se han modificado originales.
- No se certifica ausencia absoluta de bugs, fidelidad colorimétrica universal ni reproducción sostenida perfecta.
- Sin instalador ni publicación en GitHub. No se marca la versión 1.0.
- Los paquetes y compilaciones obsoletos se retiran después de validar su sustituto, conforme a la política del proyecto.
