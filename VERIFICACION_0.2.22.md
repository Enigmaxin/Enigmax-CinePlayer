# Verificación de Enigmax CinePlayer 0.2.22

Trabajo local del 01/11/2026. Base única; sin instalador ni publicación. Se mantiene el motor fijado y se recompilan ejecutable y puente MVC. No se presenta como 1.0 mientras haya validaciones importantes pendientes.

## Cambios

- Nuevo módulo de políticas opcionales `playback_features`: caché comprimida adaptativa con límites e histéresis; respeta la cola sin comprimir MVC y el cambio al modo de menor consumo.
- Reutilización del par MVC ya disponible para reposicionamiento dentro de él. Saltos cortos hacia delante con timestamps disponibles continúan decodificando desde el estado actual, evitando el reposicionamiento y preroll de cinco segundos. No se añaden cuadros de reserva ni se cambia el contenido de los ojos. Búsquedas largas y hacia atrás conservan el mecanismo anterior.
- Diagnóstico bajo demanda de lectura, presentación y formatos. La barra puede indicar espera de datos. Sin deducir avería a partir de contadores.
- Cambio opcional de frecuencia en trabajador Windows; mismos píxeles, orientación, profundidad y flags. Comprobación CDS_TEST, lectura del modo realmente aceptado, restauración condicionada al modo propio y exclusión entre instancias por pantalla. No se instala un controlador ni se inyectan modos o metadatos HDMI 3D. La negociación exacta de fracciones requiere validación del controlador; DEVMODE es una API de frecuencias enteras.
- Perfiles por identificador de pantalla: salida de color SDR/automática y audio, dispositivo disponible y retardo. No se inventan valores de calibración ni se fuerza una LUT/ICC genérica. El tone mapping del motor anterior se conserva.
- Cursor vectorial por ojo, sin capturar vídeo ni crear un lienzo bitmap de pantalla completa. Restauración del cursor nativo en 2D. Actividad por eventos y temporizador de salud cada dos segundos únicamente con reproducción cargada.
- Preferencias de audio/subtítulos por etiquetas de idioma y pistas forzadas/originales disponibles. La elección guardada por archivo tiene prioridad y las recargas internas MVC no vuelven a imponer la regla global. Una etiqueta ausente o incorrecta limita la elección; no se identifica el idioma escuchando ni analizando imágenes.
- Audio original sin filtros por defecto; modo nocturno y realce suave de voz opcionales sobre PCM. Filtros etiquetados reemplazables; cambiar subtítulos/cursor no reconstruye audio. Bitstream suspende los efectos. No se añaden HRTF ni procesadores neuronales.
- Desentrelazado automático opcional del motor, sin activarlo sobre material progresivo por defecto. Selector de ediciones declaradas en Matroska; las playlists Blu-ray siguen en Títulos Blu-ray.
- Tamaño de subtítulos y seis colores, persistentes. Personalización ASS optativa, conservando posiciones pero con limitaciones de autoría compleja. PGS puede escalarse; el color de los bitmaps no se altera con el control de texto.

## Evidencia y límites

La evidencia ejecutada y los hashes de entrega están en `verificacion/0.2.22/`.

- Compilación nativa, formato, Clippy con advertencias como errores y ejecutable release correctos. Tres pruebas C aprobadas; la prueba IO mantuvo 54 handles antes y después.
- Batería general: 50 pruebas del núcleo y 48 del ejecutable aprobadas, diez opcionales no incluidas. Entre las nuevas: límites de caché, selección por idioma, transiciones de audio/subtítulos, persistencia y recuperación tras fallo de escritura, y cierre de trabajadores de pantalla con peticiones pendientes.
- Comprobación GDI aislada aprobada: 100 ciclos de menú, contador 0 → 1 en esta ejecución. Prueba opcional MVC con Goliath: 100 solicitudes de búsqueda agrupadas, llegada al destino y cierre con destrucción del decodificador en unos 193 ms; aprobada. En total, 104 pruebas Rust distintas ejecutadas y aprobadas, incluyendo seis opcionales distintas.
- Tras los últimos ajustes se repiten Clippy y las pruebas focalizadas de políticas y píxeles. Capturas de SRT/ASS confirman aumento de área y color amarillo; se recupera el blanco del autor ASS al desactivar su personalización. PGS aumenta el área de su bitmap al pasar de 100 a 150 %. No se recolorea ni se convierte mediante OCR.
- Prueba Lua: cursor e interacción proyectados en SBS 3840×1080, HSBS 1920×1080, OU 1920×2160, HOU 1920×1080, Frame Packing y anaglifo; restauración del cursor nativo en 2D. Pruebas existentes de navegación y menús por ambos ojos aprobadas.
- Siete sondas del EXE final: Hellcat/SDR, HDR10 → SDR, Full SBS, HSBS, HOU, Frame Packing y Goliath/MVC anaglifo. Avance temporal y cierre correctos; D3D11VA en Hellcat, WASAPI en Hellcat/MVC, cero descartes del decodificador en estas muestras. Los descartes de presentación acumulados fueron 0–3; no se afirma ausencia total de descartes. El registro incluye reconfiguración del renderizador durante la apertura; una captura final no permite atribuir cada descarte ni certificar cadencia sostenida. A/V reportó unos 5–9 µs en las dos sondas con audio real silenciado: no es una medida de latencia física o sincronía labial.
- Las siete sondas conservaron por hash todos los archivos de configuración personal. Las ISO conservan tamaño y fecha; no se escribe en los originales.

- Refuerzo de estabilidad: Hellcat supera 50 aperturas en diez sesiones y 30 sesiones de cierre durante carga/sustitución, sin reproducir el bloqueo. También pasan cancelación/reemplazo asíncrono de ISO y siete cambios de calidad MVC con Goliath.
- Se prueban los tres modos de escucha en 18 muestras de audio reales sintéticas comprimidas/sin pérdida y estéreo/multicanal: se mantienen 48 kHz, canales y avance del audio. No es una escucha ni una certificación HDMI.
- La solicitud de restaurar hercios sustituye peticiones pendientes mediante un buzón de último valor: la prueba de concurrencia comprueba que una restauración no se pierde detrás de activaciones anteriores. Desactivar la lectura adaptativa o cambiar el color de subtítulos no aumenta la cola MVC de mínimo consumo.

- La prueba específica de perfiles aplica SDR y retardo por pantalla, recupera los valores generales y usa el dispositivo automático cuando el guardado no existe. No cambia la frecuencia del monitor. Clippy detectó y bloqueó una alteración de acentos al añadir la prueba; se corrigió la codificación UTF-8 y se repitieron análisis y pruebas antes de recompilar.

- Los avisos periódicos de salud y de posición conservan solo el último valor mediante propiedades observadas; no acumulan mensajes durante diálogos. Las pruebas inyectan 4000 cambios sin desbordar la cola y simulan 2000 ciclos de temporizadores; se verifica que la reanudación recibe la posición correcta y descarta datos de otro archivo.

Las pruebas de píxeles usan patrones sintéticos: no equivalen a calibración física o revisión visual completa de películas. Los contadores A/V tampoco sustituyen escucha y comprobación labial. No se repiten en esta entrega todas las pruebas opcionales extensas de discos UHD, equipos distintos o películas completas.

La nueva prueba nativa lee Goliath y Friday the 13th en E: sin modificar tamaño o fecha. Los pares completos de los fotogramas 0, 1, 24 y 48 se contrastan por SHA-256 entre lectura secuencial y búsquedas. En ambas ISO, reposicionar dentro del par produjo cero lecturas; los saltos cortos observados leyeron unos 0,5 MiB lógicos frente a 3,3/10 MiB de las búsquedas de referencia. Caché del sistema y carga no controladas: no es una promesa de aceleración general ni una prueba de salud del disco.

## Pendiente, sin afirmar implementación

- Índice CLPI para búsquedas MVC largas: las posiciones de M2TS y SSIF no son intercambiables. Falta integrar y validar el mapa de intercalado y accesos de ambas vistas con discos multiclips; no se añade una búsqueda aproximada que comprometa integridad o sincronía.
- OpenXR nativo sigue siendo una línea futura; esta versión conserva la salida de escritorio compatible con los modos admitidos por el visualizador.
- Verificación física de cambio/restauración de hercios, fracciones 23,976/59,94, cambio entre pantallas HDR/SDR, receptores HDMI y cursor en Quest/Virtual Desktop. El código no certifica estas cadenas.
- Menús BD-J, descifrado de discos, HDMI Frame Packing real, Dolby Vision FEL completo y metadatos HDR10+ nativos enviados al visualizador no se incorporan en esta versión.
- No se garantiza reproducción sin incidencias de cualquier archivo corrupto, disco averiado o equipo sin capacidad de decodificación suficiente. No se reduce resolución, profundidad de vídeo o calidad de audio para ocultar esa limitación.

La condición del usuario para numerar 1.0 exige validar las funciones importantes en su uso real. Se conserva 0.2.22 mientras subsistan estos puntos; pasar pruebas automatizadas no autoriza afirmar funcionamiento perfecto universal.
