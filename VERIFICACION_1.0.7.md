# Enigmax CinePlayer 1.0.7: subtítulos y controles

Entrega local del 23/11/2026. No se publica en GitHub en esta tarea.

## Cambios

- Mientras se muestra la barra se reserva espacio vertical para SRT y PGS
  convencionales. La posición aplicada es temporal; el menú y el guardado usan
  el porcentaje elegido por el usuario. Al ocultar la barra se recupera ese valor.
- El desplazamiento no se acumula al mostrar la barra repetidamente o redimensionar
  la ventana. Si se cambia la posición manual durante la visibilidad, se conserva
  esa nueva preferencia al ocultarla y al volver a abrir el programa.
- ASS/SSA no reciben el desplazamiento temporal: se conserva su composición,
  incluidas las posiciones por ojo. Al cambiar a ASS o desactivar subtítulos se
  retira el desplazamiento temporal de la pista anterior.
- La barra permanece visible mientras se sitúa el puntero sobre sus controles
  o se arrastra uno. Se oculta aproximadamente 450–550 ms después de salir,
  usando el temporizador existente. No reaparece continuamente al mover el
  puntero fuera de su zona; se vuelve a activar al entrar o pulsar sus controles.
- Las regiones de interacción se comparten con la detección de permanencia en
  la barra, incluidas ambas vistas de OU y Frame Packing.

No se modifican las DLL, los decodificadores, los formatos de audio ni los
algoritmos de color. El mecanismo cambia una propiedad de composición de
subtítulos; no lee los píxeles del vídeo durante el uso normal ni reconstruye
el grafo de filtros o el shader de vídeo. Los porcentajes internos de separación
residen en memoria de la instancia de mpv y no en los archivos de preferencias.

## Pruebas y resultados exactos

- Formato Rust y Clippy offline, con advertencias como errores: aprobados.
- Batería general secuencial: 57 pruebas de biblioteca aprobadas; en aplicación,
  71 aprobadas, 1 fallida y 16 especiales omitidas. El único fallo fue el nuevo
  ensayo de continuidad: su configuración de prueba arrancaba en pausa y por
  ello no avanzaba el reloj (`Playback did not keep pace`).
- Se corrigió exclusivamente ese arranque de la prueba y se repitieron las tres
  pruebas nuevas: 3 aprobadas, 0 fallidas. No se cambió el código de producción
  tras la batería general. No se presenta la primera ejecución como totalmente
  aprobada ni se borran sus registros. Resultado conjunto: los 129 tests distintos
  ejecutados cuentan con una ejecución aprobada; 16 requieren pruebas aparte.
- La prueba de píxeles cubre 24 combinaciones: PGS, SRT y ASS en 2D, Full-SBS,
  Half-SBS, Full-OU, HOU, SBS anamórfico, anaglifo y composición Frame Packing.
  Verifica que los SRT/PGS siguen visibles fuera de la zona de controles,
  que la imagen original se restaura, que ASS permanece idéntico, y que no
  cambian las pistas, el reloj pausado ni el grafo de vídeo.
- La prueba de continuidad con 30 ciclos de mostrar/ocultar/redimensionar avanzó
  3,708 segundos de vídeo durante 3,710 segundos de prueba. Conservó filtros,
  shaders, pistas y porcentaje base, sin acumular desplazamiento.
- Prueba Lua dentro del motor incluido: primera pulsación, pausa/doble clic,
  navegación modal, permanencia al mantener el puntero, salida con plazo breve,
  arrastre fuera de la barra y regiones de los dos ojos: aprobada.
- Las 153 transiciones de perfil de la batería general pasaron, junto con las
  pruebas existentes de color, audio, menús, búsqueda, subtítulos y cierre.
- Compilación release aprobada; Cargo y recursos PE indican 1.0.7.
- ZIP portátil: CRC, manifiesto y SHA-256 de los 141 archivos verificados desde
  extracción limpia con PATH limitado a Windows. Cuatro casos terminaron con
  código 0 y captura de controles: PGS 2D, PGS SBS anamórfico, SRT con audio y
  ASS 3D original. Se inspeccionaron visualmente las capturas SRT y PGS.
- Ejecutable activo idéntico al del ZIP. Cargo.lock solo cambia la versión de
  la aplicación; las dependencias y bibliotecas nativas se conservan.

## Límites

No se han probado todos los archivos, dispositivos, controladores o combinaciones
de subtítulos. Los PGS pueden contener gráficos, rótulos superiores o imágenes
de pantalla completa; el motor conserva sus reglas de posicionamiento y este
mecanismo no analiza ni reescribe esos mapas. Una ventana extremadamente pequeña
o subtítulos que ocupen gran parte de ella no permiten garantizar espacio libre.
ASS conserva su ubicación aunque coincida con la barra: para verlo de nuevo hay
que apartar el puntero de los controles y dejar que estos se oculten.

Las mediciones se limitan al equipo de prueba; no certifican Quest 3/Virtual
Desktop ni MVC sobre una ISO real en esta tanda. Frame Packing sigue siendo
composición experimental, sin activación HDMI 3D. No se garantiza ausencia
absoluta de errores. Registros completos en `work/validacion-1.0.7/`.
