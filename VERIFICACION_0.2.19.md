# Verificación de Enigmax CinePlayer 0.2.19

Versión preparada localmente el 29/10/2026. No publicada en GitHub en esta tarea.

## Cambios

- Calidad / consumo pasa al menú principal, inmediatamente después de Color / HDR. Se conservan los tres perfiles anteriores y sus índices guardados.
- Mínimo consumo incorpora escalado bilineal, desactiva el cálculo dinámico del pico HDR y limita la caché de lectura a 16 MiB (8 MiB con MVC), sin caché hacia atrás. Respeta límites previos más pequeños y restaura los límites reales de cada transporte al cambiar de perfil. Mantiene resolución, frecuencia, política de decodificación y procesamiento de audio. La menor caché puede aumentar las lecturas o reducir la tolerancia a interrupciones de entrada; el escalado y la estimación HDR difieren de los perfiles superiores.
- Los menús comparten sus acciones: clic derecho y tres puntos, Audio, Subs y el menú 3D completo. La ventana visual 3D conserva sus tarjetas y añade Todas las opciones 3D. Incluye Reproducir en SBS Anamórfico.
- Audio y Subs muestran idioma y códec/formato de cada pista desde sus metadatos. Los datos ausentes se indican sin inferirlos del título.
- Al arrancar sin archivo explícito, el último vídeo disponible se carga en pausa en la posición guardada. Su título acompaña a Continuar reproducción y Empezar de nuevo. Sin un último archivo disponible aparece Arrastra tu vídeo aquí. Se evita sobrescribir la posición mientras se espera la elección.

## Comprobaciones ejecutadas

- build.ps1: formato, Clippy con advertencias como errores, compilación nativa y compilación release correctas.
- 81 pruebas Rust ordinarias: 50 del núcleo y 31 del ejecutable; cero fallos. Otras seis pruebas opcionales están excluidas de esta ejecución ordinaria.
- Prueba GDI aislada y prueba opcional de reanudación ISO: ambas correctas. Total: 83 pruebas Rust ejecutadas y aprobadas, más dos pruebas C.
- Paridad de todas las acciones de menú, ubicación de Calidad / consumo, idiomas y códecs de pistas sintéticas, conservación de perfiles y restauración de límites de caché.
- Arranque real con mpv/Lua: pausa, continuación, reinicio, teclado y redibujado tras recarga. Un argumento de archivo explícito conserva precedencia.
- Wolf Warrior.iso: sustitución del MKV anterior, título, pistas y posición guardada en transporte nativo y MVC; posiciones observadas 123,498367 s y 123,500000 s. Comprobada la restauración de caché al salir de Mínimo consumo.
- Sondas release SDR y anaglifo Full SBS con perfiles Mínimo consumo y Máxima calidad, además de ruta CPU y equilibrada. Captura del arranque con pausa activa.
- Los seis archivos de configuración personal inventariados conservan sus tamaños y SHA-256 antes y después de las sondas.

Evidencia local: verificacion/0.2.19/build-final.log, iso-startup-low.log, capturas y sus informes de propiedades, config-before-probes.json y config-after-probes.json. La validación de los paquetes se registra por separado en delivery.json después de crearlos.

## Límites de la validación

Pruebas realizadas en Windows 11. No se ha medido ahorro de CPU/RAM ni fluidez en un Ryzen 3 con 4 GB, ni se certifica Windows 10 a partir de estas pruebas. Las capturas y comparaciones automatizadas no certifican la salida física HDR/HDMI o todos los dispositivos 3D. Cuatro pruebas opcionales de estrés no se han ejecutado en esta revisión. Los resultados describen los escenarios comprobados y no garantizan ausencia universal de fallos.
