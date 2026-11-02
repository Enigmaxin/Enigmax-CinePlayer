# Verificación de Enigmax CinePlayer 1.0.1

Fecha: 2 de noviembre de 2026. Corrección de menús sobre la base vigente. No se crea instalador ni se publica en GitHub.

## Cambios autorizados

- Clic derecho en 2D y anaglifo: vuelve a utilizar el menú nativo de Windows con el diseño M existente, junto al puntero, y submenús en cascada. Anaglifo muestra el menú plano y con sus colores normales. No pasa por la conversión del vídeo.
- SBS, HSBS, OU/HOU y Frame Packing conservan los menús del reproductor proyectados por ojo. Los desplegables de los controles inferiores mantienen su ruta anterior; esta entrega recupera específicamente el menú contextual nativo en 2D/anaglifo.
- Posición de subtítulos pasa a ser una lista de 50 a 110 %, en pasos de cinco puntos, con el valor actual marcado. Se aplica y cierra al elegir, igual que Tamaño. Se retiran Subir/Bajar, el porcentaje informativo separado y cualquier consulta de información de altura. Restablecer aspecto sigue recuperando tamaño, color y posición 100 %.
- El ajuste escribe sub-pos directamente y conserva los filtros y el renderizador. Los ASS con posiciones explícitas siguen respetando esas posiciones y su autoría estéreo; no se reescriben sus estilos ni los bitmaps PGS.
- Se retira la infraestructura de callbacks que mantenía abierto el menú para pulsaciones repetidas: ya no es necesaria con la lista. No se introducen filtros globales de entrada ni hilos de interfaz adicionales.
- Se cancela el menú nativo al perder la activación o recibir cancelación. Mientras está abierto, una comprobación temporal de su ventana de vídeo cada 100 ms evita retener el cierre si esa ventana desaparece. El temporizador y la subclase se liberan al cerrar el menú, también en alto contraste. No existe ese sondeo cuando el menú está cerrado.
- La lógica de frecuencias, decodificación, color, HDR, audio y transporte 3D no se modifica. Se conservan la apertura al 50 % con área 16:9 y las demás mejoras de 1.0.0.

## Pruebas

- build.ps1 final: formato Rust, Clippy con -D warnings, cuatro pruebas C, 53 pruebas del núcleo y 56 de aplicación aprobadas; compilación release y sustitución del EXE habitual correctas.
- Selección real mediante libmpv: trece posiciones en cinco proyecciones (2D, SBS, OU, Frame Packing y anaglifo), 65 selecciones. Se comprueban aplicación, cierre, marcado único del valor actual y conservación de vf, af y current-vo. La prueba distingue la ruta nativa contextual de la ruta de los controles.
- Recursos nativos aislados: 100 ciclos de decoración y liberación, GDI 0 → 1. Incluye alta/baja de los recursos temporales del menú. No equivale a una certificación universal de ausencia de fugas.
- Verificación visual del EXE: clic derecho junto al punto pulsado en 2D; submenús nativos; menú plano en anaglifo; cierre al pulsar fuera y finalización al cerrar la ventana con el menú desplegado. Las sesiones usan --probe y no guardan preferencias personales.
- Prueba interactiva separada del mismo renderizador nativo: cuatro niveles simultáneos, selección 75 % (ID 1145), reapertura con 75 % marcado, selección 110 % (ID 1152), acceso por Shift+F10, Escape y devolución del foco al propietario. Registro: work/101-native-interactive.log. La herramienta no instala hooks ni genera entrada global por sí misma.
- Se repiten las pruebas de calidad/consumo y sus 153 transiciones, persistencia, color SDR/HDR→SDR, audio, ASS/SRT/PGS, cursor, controles estéreo, capítulos, búsquedas, historial, cancelación y cierre.
- Una pasada concurrente no alcanzó la geometría de pantalla completa: quedó una ventana sin bordes de 982×596 frente a 3840×2160. La prueba aislada posterior pasó con gpu y gpu-next a 3840×2160, DPI 144, y la repetición completa final pasó sin fallos. No se ha demostrado la causa de aquel resultado; no se presenta como una reparación del motor de pantalla completa.
- EXE final: 11 reproducciones supervisadas (incluidas tres aperturas de Hellcat), siete casos válidos/dañados y diez mutaciones de MKV. Todos terminan dentro del límite; los truncados son rechazados de forma controlada.
- Ocho archivos de configuración personal conservan sus hashes. Se verifican las 125 DLL contra el manifiesto; las dependencias del motor no cambian. El ZIP se comprueba por extracción y hashes. El arranque del portátil se prueba con PATH limitado a Windows; el resultado queda en verificacion/1.0.1/portable-clean.json.

## Calidad y recursos

La ruta contextual nativa no crea los bitmaps por ojo del menú gráfico en 2D/anaglifo. La selección de altura no reconstruye el pipeline de vídeo o audio. Los menús gráficos siguen dibujándose por eventos en las salidas que necesitan proyección estéreo. Se evita añadir un mecanismo especial de menú persistente, un motor web o una nueva dependencia.

No se atribuye una mejora numérica de CPU/RAM a esta entrega: no se ha ejecutado una comparación estadística de recursos contra 1.0.0. Las comprobaciones de recursos, invariantes de filtros y código permiten acotar los cambios; no garantizan el menor consumo posible en todo equipo.

## Límites y entrega

No se certifica ausencia universal de bugs ni comportamiento idéntico en todos los controladores y pantallas. No hay nueva certificación física Quest/Virtual Desktop, HDR nativo, bitstream HDMI ni ISO MVC/UHD no disponibles. El caso previamente señalado de modificación manual de frecuencia entre vídeos sigue pendiente de una revisión específica; la lógica de frecuencias no se ha tocado en esta corrección de menús. Las propuestas de supervisor independiente y confirmación de modo de pantalla no están implementadas.

El informe de entrega y los hashes se guardan en verificacion/1.0.1/delivery-status.json. Los paquetes obsoletos se retiran después de validar los nuevos; se mantienen los informes históricos. Las fuentes de dependencias se reutilizan con su hash verificado, sin recompilarlas ni recomprimirlas.
