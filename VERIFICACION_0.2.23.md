# Enigmax CinePlayer 0.2.23 — correcciones y auditoría

Fecha: 1 de noviembre de 2026. Base única vigente: este directorio, cuyo nombre histórico no indica la versión.

## Cambios

- Cursor estereoscópico mediante una franja bitmap transparente en el ID 63, por encima de los menús (59/60) y controles (61/62). Dos posiciones por ojo, sin lectura de vídeo ni textura del tamaño de toda la ventana. Coordenadas y tamaños acotados; movimiento comunicado con una propiedad observada que agrupa eventos. Los menús actualizan el cursor mientras su bucle ocupa el cliente principal. Se atiende movimiento también en las secciones inferiores de entrada y se limpia el cursor al cambiar de archivo.
- Cursor integrado solo para SBS, OU y Frame Packing. Anaglifo y 2D recuperan la política del cursor de Windows.
- Tamaño y color exclusivamente para pistas SRT. ASS/SSA conserva escala 1, estilos y posiciones originales, incluidos los pares 3D; se retira el ajuste que forzaba estilos ASS. PGS conserva su bitmap original. El procesamiento de profundidad estereoscópica existente no se elimina.
- Se elimina la segunda escritura de `sub-ass-override` desde la ruta de vídeo; la política de apariencia tiene un único responsable.
- Desactivado el redimensionado automático de la ventana al reconfigurar el vídeo. Se captura y restaura WINDOWPLACEMENT cuando hace falta cambiar entre gpu/gpu-next fuera de pantalla completa.
- Audio incluye Filtros: Original, Modo nocturno y Claridad de voz. Los filtros se suspenden con bitstream; no se recodifica el flujo para aplicarlos.
- Retirados diálogos y atajos de sincronización manual de audio/subtítulos. Los perfiles antiguos ya no restauran el retardo manual de audio. Se conserva el ajuste temporal interno del transporte MVC: no es un control manual.
- Reproducción contiene exclusivamente lectura adaptativa, frecuencia automática, cursor estéreo y desentrelazado.

## Validación ejecutada

- `build.ps1` vigente: tres pruebas C correctas, formato Rust, Clippy con advertencias como errores, 50 pruebas del núcleo y 50 del ejecutable aprobadas. Compilación release terminada. IO cancelable: 48 handles antes y después.
- Prueba GDI aislada de 100 ciclos de menú aprobada (0 → 1).
- Prueba adicional aislada de 10 ciclos de motor con dependencias residentes aprobada: tras el calentamiento, kernel/GDI/USER permaneció en 1135/12/2 hasta el último ciclo. En total, 102 pruebas Rust distintas aprobadas, contando las dos aisladas. Nueve pruebas opcionales permanecen sin ejecutar en esta entrega.
- Comparación de píxeles: SRT crece y cambia a amarillo; ASS no cambia al variar las preferencias de apariencia; PGS mantiene su bitmap. No demuestra que todo ASS imaginable sea correcto: por ese motivo se ha retirado la personalización de estilos ASS.
- Alternancia repetida SRT/ASS con cambio real de gpu-next/gpu: se mantienen posición y dimensiones de ventana. El cursor bitmap aparece por encima de los planos opacos de controles en la captura.
- Pruebas Lua de ciclo de menús y cursor: SBS completo/medio, OU completo/medio, Frame Packing y exclusión de anaglifo. Generación de 540 combinaciones de modo/dimensiones, con coordenadas válidas y fuera de ventana. Son pruebas controladas, no validación con gafas físicas.
- Estrategia adicional de recorrido de estados con orden pseudoaleatorio reproducible: 40 transiciones de pista, modo, pausa y seek, recorriendo Full SBS, HSBS, OU y HOU y cinco salidas. Se conservan geometría, pausa y primarias BT.709; no desborda la cola de eventos.
- Se repiten además las pruebas existentes de códecs de audio, cambio de pistas, profundidad de subtítulos, HDR→SDR, menús, cierre y errores de estructuras de disco. Decodificación contra sumidero nulo no certifica salida HDMI física ni escucha humana.

## Hallazgo adicional: descarga de dependencias

Al crear y destruir motores **descargando y volviendo a cargar sus DLL entre ciclos** se observó crecimiento sostenido de handles: aproximadamente 24 por ciclo con GPU y 21 con salida null, incluso con un segundo de espera. GDI/USER no crecieron. Manteniendo cargada la misma biblioteca, el crecimiento desapareció después del calentamiento.

La aplicación normal crea un único Mpv en `run()` y mantiene sus dependencias cargadas hasta salir. No se ha demostrado una fuga equivalente durante el cambio normal de película. La causa concreta dentro de las dependencias/runtime sigue sin identificarse; no se han añadido cierres manuales de handles ajenos ni cambios de arquitectura especulativos. La prueba opcional puede reproducir la recarga de DLL con `ENIGMAX_AUDIT_UNLOAD=1`; `ENIGMAX_AUDIT_VO=null` permite separar el renderizado GPU. Los registros de ambas variantes se conservan como evidencia.

## Validación final del EXE y entrega

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

## Límites

- Sin validación física de Virtual Desktop/Quest, receptores HDMI, HDR o todas las pantallas/DPI.
- Sin nueva campaña sobre ISO grandes: TALKTOME y Titanic no estaban disponibles en las rutas autorizadas. Hellcat sí se ha probado de nuevo; no se han modificado originales.
- No se certifica ausencia absoluta de bugs, fidelidad colorimétrica universal ni reproducción sostenida perfecta.
- Sin instalador ni publicación en GitHub. No se marca la versión 1.0.
- Los paquetes y compilaciones obsoletos se retiran después de validar su sustituto, conforme a la política del proyecto.
