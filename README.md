# Enigmax CinePlayer 0.2.7 — Windows 11 x64

Versión de desarrollo funcional. Se mantiene únicamente la última versión del programa; las versiones obsoletas se eliminan por indicación del usuario. Logo A elegido: cinta dorada y símbolo de reproducción azul.

Corrección MVC incorporada desde 0.2.3: **VERIFICACION_0.2.3.md**. Se reconstruyen las unidades NAL fragmentadas entre paquetes PES del SSIF para evitar corrupción del ojo derecho, comprobada con TALKTOME_3D.iso. Las mejoras de interfaz y la batería previa de sincronización están documentadas en **VERIFICACION_0.2.2.md**.

## Abrir y reproducir

Extrae el ZIP completo y ejecuta `EnigmaxCinePlayer.exe`. Conserva `libmpv-2.dll` y la carpeta `runtime/`. No necesitas instalar Rust ni FFmpeg para utilizarlo. Arrastra un MKV, MP4 o ISO, o pulsa **Ctrl+O**. Para abrir por doble clic, elige el ejecutable mediante «Abrir con» de Windows. Las ISO se leen directamente; no se montan ni se modifican.

La versión aparece en el título, en **Ayuda / F1** y en **Acerca de** dentro del menú general. Se obtiene de Cargo.toml al compilar.

**M / clic derecho:** menú. **Ctrl+3:** vídeo 3D. **Espacio:** pausa. **F / doble clic:** pantalla completa. **Esc:** ventana. **1:** tamaño original, sujeto a los límites de pantalla. **Flechas:** buscar. **Ctrl+S:** subtítulos externos. **Q:** cerrar.

## Presentación estereoscópica

Al abrir cada vídeo reconocido como 3D se pausa temporalmente y se pregunta cómo verlo: **2D sin gafas**, **anaglifo** con las gafas elegidas o **dos vistas originales** para una pantalla 3D. Se pregunta de nuevo al abrir otro archivo o reabrir el mismo; un cambio interno de transporte MVC conserva la elección. Al cerrar la pregunta sin elegir se utiliza 2D izquierdo.

**Vídeo 3D / Ctrl+3** reúne las opciones habituales: **Ver en 2D**, **Ver con gafas de colores**, **Mostrar las dos vistas originales** e **Invertir profundidad**. Elegir las gafas activa directamente el anaglifo. Hay cinco parejas: rojo/cian, verde/magenta, ámbar/azul, rojo/azul y rojo/verde; estas dos últimas son monocromas. **Formato del archivo** corrige la detección. **Avanzado** agrupa el ojo 2D, método de color, calidad, calibración y ASS 3D.

El modo automático reconoce etiquetas explícitas del nombre como Full-SBS, HSBS, Full-OU, HOU y MVC, metadatos inequívocos y títulos Blu-ray con vista dependiente. Un nombre genérico «SBS», «3D» o unos metadatos sin información suficiente sobre Full/Half abren primero la confirmación del formato. No se deduce Full/Half por la resolución ni se analiza toda la imagen; un archivo sin etiquetas ni metadatos puede requerir selección manual. Las preferencias de formato y presentación de la película anterior se reinician para evitar aplicarlas al siguiente vídeo 2D. El menú permite corregir el aspecto final de fuentes anamórficas o recortadas.

Los controles inferiores se muestran planos en 2D. En anaglifo se compone una sola barra con las gafas seleccionadas y una disparidad cruzada suave de 2–8 píxeles según el tamaño de ventana. En original SBS/OU se envía una copia por ojo, con zonas de clic equivalentes. Se corrige el indicador estereoscópico de salida para evitar otra duplicación de la interfaz. La barra aparece al mover el ratón, se oculta tras la inactividad y actualiza como máximo unas diez veces por segundo mientras está visible. Una búsqueda con el deslizador se ejecuta al soltarlo.

La composición GPU separa los ojos antes del escalado, trabaja en RGB lineal y no recodifica el archivo. La calibración permite ajustar convergencia ±3 %, ganancias de cada ojo y compensación de fugas hasta 15 %. La convergencia desplaza las vistas: no reconstruye profundidad ni elimina defectos de las gafas/pantalla.

**Equilibrado:** GPU y escalado spline36. **Máxima calidad:** GPU y EWA Lanczos. **Compatibilidad:** filtro FFmpeg stereo3d por CPU en RGB de 8 bits; no aplica la calibración GPU y no ofrece equivalencia de color exacta con ella. El modo anaglifo utiliza salida SDR. Para fuentes HDR se realiza primero conversión tonal a SDR mediante CPU; aumenta el consumo y puede limitar la fluidez en equipos modestos.

## Blu-ray e ISO

El menú **Títulos Blu-ray** muestra playlist, duración, número de clips, presencia de SSIF y repeticiones. La selección inicial prioriza un título 3D sin segmentos repetidos; en discos 2D escoge el más largo sin repeticiones. Es una heurística: comprueba el título y elige otro cuando corresponda. No se implementan menús BD-J ni descifrado AACS/BD+.

Para anaglifo o vista dependiente se utiliza un decodificador MVC real. Solo se presenta como MVC cuando se han recuperado ambos ojos. La lectura conserva audio y subtítulos del título seleccionado. El transporte usa una cola pequeña de vídeo sin comprimir; no crea una conversión temporal de la película. Para mostrar la vista base en 2D se evita decodificar la segunda vista y se mantiene la ruta nativa de menor consumo.

El soporte MVC está en desarrollo: las pruebas reales se han realizado con la ISO Hyperspace facilitada. MKV/MP4 MVC, playlists con múltiples clips y cambios de formato necesitan más muestras reales. El transporte actual exige frecuencia constante, dimensiones constantes y YUV 4:2:0 de 8/10 bits. Rechaza discontinuidades que impidan conservar la sincronización. No convierte contenido 2D en 3D.

## HDR, sonido y subtítulos

SDR y HDR10 usan gpu-next/D3D11. El modo automático depende del estado HDR de Windows y de la pantalla. También puedes forzar SDR. La ISO UHD facilitada se ha abierto como HEVC 3840×2160 PQ/BT.2020 con D3D11VA y audio TrueHD. Eso no certifica la señal enviada al televisor.

No se garantiza passthrough de metadatos HDR10+ ni salida nativa Dolby Vision, tampoco reconstrucción FEL. La interpretación de RPU/reshaping y la reproducción de una capa HDR compatible no equivalen a activar Dolby Vision en la TV. No se ha verificado físicamente una cadena HDR/HDMI.

WASAPI/PCM para altavoces o TV; selección de dispositivo y pistas en el menú. Passthrough opcional AC3/DTS o HDMI ampliado AC3/EAC3/DTS-HD/TrueHD, según receptor y controlador. No se ha comprobado con un receptor físico ni mediante escucha.

SRT y ASS/SSA conservan el renderizado nativo, incluidos estilos y fuentes adjuntas. **Nuevo: los ASS con eventos estereoscópicos duplicados se detectan y se procesan junto con el vídeo.** Consulta el apartado siguiente. Los subtítulos 2D siguen superpuestos en el plano de pantalla. Las pistas PGS del Blu-ray se enumeran y seleccionan con su decodificador nativo; la validación visual de PGS sigue pendiente y no se detectan ni transforman PGS estereoscópicos duplicados. Los controles de posición no sustituyen las posiciones explícitas de un ASS.

## Subtítulos ASS 3D

Activado por defecto: **M → Vídeo 3D → Avanzado → Subtítulos 3D → Automático**. Se comparan eventos simultáneos de la pista ASS seleccionada: mismo texto, tiempos, capa y estilo, con posiciones correspondientes en mitades opuestas. Se reconocen posiciones explícitas, movimientos sencillos y márgenes de estilos equivalentes. La identificación queda asociada a esa pista y se reinicia al cambiar de archivo o de subtítulo. No se mezclan dos pistas independientes ni se interpreta como 3D una pista solo por su nombre.

Cuando se confirma una pareja SBS u OU compatible con el formato de vídeo seleccionado:

- Anaglifo: texto y vídeo pasan por la misma matriz de gafas, intercambio de ojos, convergencia y calibración.
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

VERIFICACION_ASS3D.md documenta esta ampliación; VERIFICACION.md conserva las pruebas de 0.2.0. No se ha certificado compatibilidad universal ni distribución pública de todas las dependencias del motor. Se distribuye como ZIP portátil y se actualiza mediante GitHub Releases; los ejecutables no están firmados digitalmente.

## Actualizaciones desde GitHub

Al abrirse y desde Acerca de > Buscar actualizaciones, consulta la última release estable en un hilo secundario. Solo descarga si aceptas. La actualización utiliza el ZIP portátil completo, verifica hashes, rutas, versión y arquitectura, espera al cierre y sustituye EXE y bibliotecas conservando config/. No ejecuta instaladores ni modifica asociaciones. Ante una sustitución fallida intenta recuperar los archivos anteriores. Consulta ACTUALIZACIONES.md para requisitos, límites y publicación.

`build-portable.ps1` genera un ZIP con los archivos necesarios para ejecutar la aplicación y sus licencias; excluye código, herramientas, pruebas y ajustes personales. El código se mantiene por separado en el repositorio. La preparación de actualizaciones usa almacenamiento temporal, pero las bibliotecas de reproducción permanecen en su carpeta y no se extraen a TEMP para ejecutarlas.

## Cambios de 0.2.7

Distribución y actualización del conjunto portátil, recuperación ante archivos bloqueados y validación estricta del ZIP. La ventana de descarga atiende eventos durante la conexión y lectura de red. Se descarta una posición de reanudación pendiente al abrir otro archivo; los errores de presentación y ASS se registran sin diálogos que bloqueen el consumidor de eventos. La aplicación puede continuar si falla la asignación del icono. Auditoría y límites: VERIFICACION_0.2.7.md.

## Cambios de 0.2.5

En el menú Acerca de se puede buscar actualizaciones manualmente; la consulta sigue ejecutándose en segundo plano e informa del resultado. La barra inferior incluye Audio y Subs (A/S en ventanas pequeñas), con las pistas disponibles y desactivación de subtítulos. Las búsquedas, cambios de pista y controles de reproducción se envían de forma asíncrona. Las búsquedas rápidas se agrupan con una solicitud activa y una pendiente; se descartan al cambiar de archivo o cerrar. Los errores de controles se muestran sin diálogo modal y se conserva un diagnóstico acotado en config/ultimo-error-reproduccion.log. Consulta VERIFICACION_0.2.5.md para el alcance de las pruebas.
