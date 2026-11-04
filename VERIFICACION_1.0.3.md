# Enigmax CinePlayer 1.0.3 — validación local, 4 de noviembre de 2026

## Cambios

- «Paralaje / Convergencia» está dentro de Vídeo 3D y tiene botón ↔ en los controles. No aparece duplicado como entrada del menú principal. Deslizador −2 % a +2 %, botones −/+, valor visible y Restablecer; aplica sin cerrar el panel.
- El paralaje se reinicia a cero en cada apertura y no se guarda. Vídeo y subtítulos acompañan el desplazamiento; la interfaz permanece fija. ASS 3D conserva las diferencias de posición que contienen sus eventos. La ruta CPU de anaglifo no compatible rechaza el ajuste sin cambiar de perfil.
- Una propiedad atómica del motor actualiza el parámetro GPU; los movimientos sucesivos no recargan archivo, decodificador, pistas ni variantes del shader. La preparación inicial del panel sí puede instalar su paso GPU. Si se cierra a cero se retira el paso adicional opcional.
- Se corrige la compensación de proporción de subtítulos convencionales en SBS/OU/anamórfico y la compensación ASS 3D según el aspecto de salida. Se evita aplicar dos veces la proporción del vídeo empaquetado.
- Los subtítulos convencionales de Frame Packing se proyectan después de componer el vídeo, a la resolución de presentación, excluyendo la franja central. ASS 3D y anaglifo conservan la fusión previa necesaria para su composición.
- Seleccionar un ojo 2D de una fuente empaquetada usa un shader GPU en lugar del recorte dinámico del renderizador. Se observaban paradas al alternar ese recorte. El cambio añade un paso GPU sencillo en esa ruta; no se afirma coste cero. No se fuerza otra resolución de fuente.
- La cadena de filtros se instala completa solo cuando cambia; ya no se vacía y reconstruye la misma cadena al elegir cada modo.
- Se elimina Avanzado y sus opciones pasan directamente a Vídeo 3D. El editor de brillo vuelve a ese menú. Volver es más grande en las cabeceras gráficas y en los submenús nativos con el tema del reproductor; la raíz sin padre se cierra con su cruz/Escape.

## Pruebas y evidencia

Registros completos: `work/validacion-1.0.3` del proyecto.

- Núcleo: 56 aprobadas, 0 fallidas.
- Primera batería completa de aplicación: 64 aprobadas, 2 fallidas, 14 optativas excluidas. Una buscaba el submenú eliminado Avanzado y se actualizó al recorrido solicitado. La otra detectó una parada real del avance temporal al seleccionar ojo; no fue una simple diferencia de metadatos.
- Antes del cambio a selección GPU, la prueba de ojos falló también aisladamente: 1 fallo de 3 repeticiones iniciales y 3 de 3 con trazas. Evitar reconstruir filtros y aplicar un único recorte no bastó: esas tentativas se conservan en los registros. Con la selección GPU, 3 repeticiones consecutivas aprobaron las ocho muestras SDR/HDR y SBS/OU, en reproducción y pausa. Esto apoya el cambio de ruta; no demuestra que se hayan eliminado todas las posibles causas de parada.
- Tras el cambio final se ejecutaron 12 pruebas focalizadas de aplicación, todas aprobadas: 153 transiciones de calidad, tres pruebas de paralaje, navegación de menús, pistas ASS, cinco pruebas de renderizado y cambios de gafas/inversión/2D. No se presenta como una nueva pasada completa de las 80 pruebas de aplicación.
- Paralaje: diez combinaciones de formato/salida en pausa, comprobando cambio de píxeles con ambos signos y restitución exacta al volver a cero. Cinco proyecciones con selección desde el segundo ojo, Volver y reapertura. 160 cambios durante reproducción con conservación de fuente, pistas y avance temporal; tiempos originales en el registro. La latencia de la llamada FFI no mide el instante físico de presentación HDMI.
- Recorrido con semilla de 40 cambios de modo/pista/pausa/búsqueda aprobado. Se comprobó avance de audio mientras permanecía abierto el menú estéreo.
- Geometría: matriz exploratoria de 99 casos y 47 comprobaciones focalizadas finales en patrones HD, estas últimas sin discrepancias con el criterio usado. Cubren selección izquierda/derecha, Full/Half SBS, Full/Half OU, Frame Packing, anaglifo HOU y PGS/SRT/ASS/ASS 3D. Se mide la proporción de patrones blancos o por canal en anaglifo, con tolerancia del 12 % por rasterización. El método se corrigió para no recortar subtítulos situados en los márgenes ni medir solo la intersección blanca de un anaglifo. Las capturas de ojo de muy baja resolución pueden perder detalle al fusionar texto antes del escalado; no son una certificación de legibilidad universal.
- Posición de SRT y PGS y conservación de ASS: ocho rutas de salida; 65 selecciones de posición en cinco proyecciones por formato admitido en la batería inicial.
- Menú nativo: tres ciclos de Volver pulsable, reapertura, Escape y limpieza exacta aprobados. Recursos GDI: 100 ciclos, contador 0 → 1.
- Pantalla completa: comprobación inicial aprobada a 3840×2160 con gpu-next y gpu. No se modificó la política de frecuencias de pantalla.
- Formato Rust, Clippy con `-D warnings` y compilación release aprobados. La compilación de mpv conserva una advertencia GCC en `filters/f_hwtransfer.c:135`, fuera del parche: posible límite de memcpy. No se ha demostrado que sea un fallo alcanzable; tampoco se ha resuelto ni ocultado.
- Los ocho archivos de configuración/historial originales conservan sus bytes y SHA-256; no hizo falta restaurarlos. De 125 DLL comprobadas, solo cambió libmpv-2.dll. Su parche y fuentes modificadas se incluyen y se actualizan sus hashes en el inventario.

## Incidencias y límites pendientes

- Una pasada intermedia de calidad encontró `pause=yes` cuando esperaba `no`. La siguiente pasó las 153 transiciones sin cambiar el código de reproducción. No se ha demostrado su causa ni se atribuye a interacción del usuario. Se conserva `postfix-quality.log`; no se declara resuelta esa incidencia por una repetición aprobada.
- El antecedente histórico `gamma: None → auto` mantiene causa no demostrada. Las pasadas aprobadas no lo resuelven por sí mismas.
- Las ISO 3 A.M., Hyperspace y TALKTOME no estaban disponibles en las rutas previamente facilitadas. No se certifica esta entrega con esos discos reales; se comprobaron las muestras y el transporte MVC sintético disponibles. No se han probado físicamente Quest/Virtual Desktop, receptores bitstream, pantallas HDMI 3D ni una cadena HDR completa.
- El alto contraste de Windows conserva su menú nativo sin el tema ampliado; no se certifica en él la presentación del nuevo Volver. La navegación nativa por teclado sigue disponible.
- Frame Packing es composición 1920×2205: no activa la señalización HDMI 3D. No hay garantía universal de ausencia de errores, compatibilidad con todos los subtítulos ni consumo mínimo en cualquier equipo.

Entrega portátil local, sin instalador ni publicación GitHub automática. Los resultados corresponden al equipo y a las muestras indicadas.
