# Verificación de Enigmax CinePlayer 0.2.21

Preparada localmente el 30/10/2026 a partir de la auditoría de 0.2.20. No se crea instalador ni se publica en GitHub. Se conserva la base única de código.

## Correcciones aplicadas

| Auditoría | Tratamiento en esta versión |
|---|---|
| B01, preparación ISO/MVC en el controlador | Enumeración de títulos, preparación MVC y verificación del transporte alternativo se ejecutan en un único trabajador bajo demanda. Cola de solicitud más reciente y canal de resultados acotados; identificación de solicitud y rechazo de resultados obsoletos. No se crea un trabajador por cada apertura. Los modos de sonda conservan la preparación síncrona para pruebas reproducibles. |
| B02, cancelación de lectura | IO propio de archivos/ISO utiliza lecturas Windows OVERLAPPED, señal de cancelación, CancelIoEx y finalización de la petición antes de reutilizar o liberar su búfer. Los callbacks de datos ya no dependen de un AVIO de archivo sin cancelación. Se conserva la caché normal de Windows. |
| B03, estado parcialmente modificado | Se preparan y validan títulos y decodificador antes de sustituir la sesión. Un fallo de preparación deja el archivo anterior activo; la acción recupera su presentación previa. Se conservan posición pendiente, pausa y elección inicial al aceptar el resultado asíncrono. |
| B04, error confundido con EOF | Se distingue EOF de fallos UDF/IO y de cancelación. Se detecta una truncación posterior a la apertura. Los errores de lectura MVC no se convierten indiscriminadamente en fin de película. |
| B05, metadatos MVC | El puente expone SAR, rango, matriz, primarias, transferencia y posición de croma, además de presencia de audio/subtítulos. Se aplican etiquetas antes de la composición y se usa SAR para el aspecto por ojo. Los nombres de opciones se contrastaron con el motor fijado y las conversiones SDR con sus cabeceras libplacebo. No se realiza un tone mapping adicional a SDR 3D por este cambio. |
| B06, subtítulos MVC sin audio | Si el contenedor no tiene audio, sus subtítulos se importan por una ruta independiente. Ya no se exige una pista de audio para incorporarlos. |
| B07, actualización con salida abrupta | El auxiliar solicita cierre al controlador mediante señal y mpv_wakeup. Se guarda el estado y se ejecuta el desmontaje normal; el trabajador ya no llama a process::exit. La llamada de despertar se serializa frente a la destrucción del handle. |
| S03, menú obsoleto | Se cierra al sustituir/finalizar el archivo o cambiar la identidad de sus pistas. La identidad usa tipo/ID, no todo el JSON dinámico de track-list, para evitar cierres por variaciones de metadatos. Los menús propios también atienden la solicitud de cierre del actualizador. |
| S04, registro FFI parcial | El propietario del registro vive antes de registrar el primer protocolo, incluso si falla el segundo. Se rechaza un registro duplicado sin sustituir su propietario. |
| S05, restauración de posición pendiente | Hay un plazo de recuperación y salida de la espera si la búsqueda no se resuelve; no se bloquean indefinidamente los controles iniciales. |

Cada motor conserva su propio almacenamiento de error: una lectura cancelada de una sesión antigua no sobrescribe el diagnóstico de la actual. Se mantiene la ABI histórica EC_Info para clientes de diagnóstico anteriores y se comprueban los nuevos exports internos al cargar el puente. Las variables atómicas de cancelación se crean y liberan en C, evitando asumir identidad de representación de atómicos Rust/C.

No se cambian los códecs, las revisiones fijadas de mpv/FFmpeg/libplacebo ni las dependencias Cargo. Se recompilan EXE y cine_mvc.dll. No se portan todavía interfaz o motor a otros sistemas.

## Verificación ejecutada

- build.ps1: formato Rust, Clippy con advertencias como errores, compilación nativa y release correctos.
- 92 pruebas Rust ordinarias aprobadas: 50 del núcleo y 42 del ejecutable; diez pruebas opcionales excluidas de esa ejecución ordinaria.
- Ocho pruebas opcionales distintas ejecutadas y aprobadas: GDI aislado, carga/cancelación asíncrona ISO/MVC, reanudación ISO, búsquedas/cierre MVC, cambios de calidad MVC, apertura/búsqueda/cierre UHD y las dos pruebas de estrés Hellcat. Total: 100 pruebas Rust distintas. Las repeticiones y procesos auxiliares de persistencia no se suman como pruebas distintas.
- Tres pruebas C aprobadas: ensamblado Annex B, detección NAL y IO cancelable. La nueva prueba utiliza una lectura de pipe pendiente en el kernel, la cancela desde otro hilo y espera la devolución de su búfer; comprueba EOF, truncación y límites de seek. El contador de handles pasó de 74 a 74 en la ejecución registrada.
- Regresiones nuevas: fallo ISO sin sustituir la sesión, última solicitud prevalece, fallo MVC con recuperación de modo, etiquetas de color y SAR recibidos por el renderizador, subtítulos incrustados sin audio, recuperación de un seek fallido, despertar/cierre seguro del actualizador y cierre de menús durante sustitución o actualización.
- Menús SBS/Half SBS/OU/HOU: navegación desde ambos ojos, submenús, gafas, retorno y segunda pulsación de 3D aprobados. El vídeo/audio continuaron durante el menú en la prueba existente.
- Se vuelven a ejecutar las 153 transiciones de calidad de la batería ordinaria y siete adicionales con MVC. Estas pruebas no certifican una transición siempre imperceptible.
- Hyperspace_3D.iso, en lectura: carga asíncrona, cambio 2D/MVC, cancelación/reemplazo y cierre. Reanudación en ambas rutas a 123,500000 s, playlist 3. Cien solicitudes de búsqueda MVC; en una ejecución el cierre incluida la destrucción del decodificador tardó 201,188 ms.
- Jurassic World - El Renacer - 2025 - 4K UHD - DV.iso, en lectura: cinco repeticiones aprobadas de la secuencia de búsquedas, cambios de pistas y cierre. No se reprodujo en esos ensayos el timeout histórico de cierre.
- Hellcat, en lectura: 50 cargas en diez sesiones con búsquedas y cierre, más 30 sesiones de cierre durante aperturas encadenadas. Las pruebas finales pasaron. En una iteración inicial hubo metadatos temporalmente no disponibles tras cambiar el filtro; la prueba espera ahora una instantánea nueva, mantiene un plazo y exige los valores correctos de color/códec.

Las sondas adicionales del EXE release y los hashes de entrega se registran respectivamente en verificacion/0.2.21/release-probes.json y delivery.json. Las configuraciones personales se contrastan mediante tamaños y SHA-256 antes/después; las pruebas de persistencia utilizan carpetas temporales aisladas.

El EXE release reprodujo Hellcat SDR y la ISO MVC con WASAPI/PCM sin escucha, y el patrón HDR10 con salida SDR. SDR/HDR mantuvieron D3D11VA; MVC decodificó por software y presentó mediante gpu-next. Los contadores acumulados de descartes fueron 0, 1 y 2 en esas sondas breves: no se afirma ausencia de descartes. Los valores instantáneos avsync fueron 0,000004 s y 0,000005 s para SDR/MVC; no prueban el alineamiento semántico del diálogo ni sustituyen una medición continua. Otra sonda del arranque real del EXE restauró la ISO en playlist 3 a 123,498367 s y esperó en pausa la elección del usuario.

## Análisis estático nativo

Se ejecutó GCC -fanalyzer sobre cine_mvc.c. Emitió una advertencia de posible byte no inicializado en be32 durante lectura MPLS. Se revisó el recorrido: solo se analiza el búfer tras exigir tamaño mínimo y lectura completa; los bytes proceden de APIs externas de lectura. La advertencia no constituye una reproducción de un acceso a datos sin inicializar y se conserva el log íntegro. No se cambió a inicialización masiva del búfer solo para ocultarla.

No se ejecutaron ASan, Application Verifier, WinDbg ni captura ETW/GPUView. Las herramientas de diagnóstico ASan/AppVerifier no estaban disponibles en la toolchain usada. Las pruebas funcionales y el conteo acotado de handles no equivalen a una certificación de ausencia de fugas o carreras en todas las DLL y drivers.

## Límites y asuntos pendientes

- S02: sigue pendiente una muestra MKV/MP4 MVC con diferencia conocida entre el primer timestamp de vídeo y el inicio del contenedor. No se aplica una compensación A/V no validada. Las pruebas ISO aprobadas no descartan ese caso particular.
- S01: cinco repeticiones correctas no demuestran haber eliminado la causa del antiguo bloqueo intermitente UHD. Si reaparece, se necesita dump de todos los hilos para atribuirlo a IO, decodificador, audio o GPU.
- La cancelación depende de que Windows y el controlador devuelvan una petición cancelada. Se espera su finalización antes de liberar su memoria; no se promete un límite duro frente a un driver bloqueado. Abrir handles y consultar metadatos de sistema también dependen del almacenamiento.
- El trabajador no llama a mpv y conserva sus propios recursos. El cierre espera brevemente su terminación; si el sistema no completa IO, se evita convertir esa espera en un join indefinido del controlador. Eso no autoriza a descargar una DLL mientras el trabajador la use.
- La recuperación transaccional cubre la preparación. Una carga aceptada por mpv puede fallar después en el demuxer/decodificador; en ese caso se informa del error, sin afirmar que pueda recuperarse automáticamente el decodificador anterior ya destruido.
- Se recuperan las etiquetas principales de vídeo MVC, no metadatos dinámicos HDR10+/Dolby Vision ni una cadena HDR estereoscópica certificada. El transporte continúa exigiendo frecuencia, dimensiones y formato constantes; no se certifica cualquier playlist multiclip.
- Las sondas WASAPI se realizan sin escucha y no certifican passthrough HDMI, receptor físico ni salida HDR física. No se han medido vatios ni se afirma una reducción universal de CPU/RAM.

Se preservan originales de vídeo/ISO/subtítulos. No se publican archivos ni se cambia la configuración del sistema para activar herramientas de diagnóstico residentes.
