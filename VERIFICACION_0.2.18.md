# Enigmax CinePlayer 0.2.18

## Cambios y causas identificadas

El historial anterior solo almacenaba posición, duración y playlist: no existía persistencia de las pistas de audio ni de subtítulos. Ahora se guardan por identidad del archivo (ruta, tamaño y modificación), independientemente de que se conserve una posición de reanudación. Se recuperan audio, subtítulos internos, desactivados y archivos externos existentes. Se identifica la pista por sus datos de origen, idioma y nombre, además del número, para tolerar cambios de IDs durante una recarga MVC. Si una pista o archivo externo ya no existe se conserva la selección disponible del motor.

El último vídeo dependía de las dimensiones decodificadas y una escritura tardía podía sustituir la de una apertura posterior. Ahora se acepta también una pista de vídeo confirmada, se comprueba la correspondencia con el archivo/transporte actual y se ordenan las aperturas por una marca temporal. Un bloqueo entre procesos protege la escritura atómica. El historial se combina con el archivo actual de disco antes de escribir, para no eliminar los registros de otras instancias. Una instancia que estaba inactiva también relee las elecciones actuales al abrir o reanudar un vídeo.

El actualizador utiliza un manifiesto público Enigmax-CinePlayer-update.json, adjunto a la release, y no consulta la API REST de GitHub. El empaquetador lo genera con versión, URL exacta del ZIP, tamaño y SHA-256. Se conservan las validaciones HTTPS, de propietario/ruta, hashes, manifiesto de contenido, arquitectura y versión PE, recuperación ante fallos y preservación de config y archivos del usuario.

La caché persiste entre reinicios: consultas automáticas cada 12 horas tras una respuesta correcta o ausencia de release; 15 minutos ante fallos; un mínimo de 60 segundos entre consultas manuales. Los límites 403/429 incorporan Retry-After y X-RateLimit-Reset, también para consultas manuales. El estado y la espera se comunican con un enlace de descargas. Un 404 se trata como ausencia de una versión con manifiesto publicado.

## Verificación ejecutada

- build.ps1: formato, Clippy con advertencias de código como errores, 46 pruebas de núcleo, 28 de aplicación, una prueba GDI aislada y compilación release correctos. Dos pruebas nativas C también correctas. Se omiten seis pruebas opcionales en la batería ordinaria; GDI y reanudación ISO se ejecutan expresamente. Total: 76 pruebas Rust correctas contando estas dos ejecuciones expresas.
- Reapertura real mediante libmpv con dos audios y dos subtítulos internos: se guardan y recuperan audio 2/subtítulos 2, subtítulos desactivados y un SRT externo con espacios en la ruta. También se comprueba desde una instancia creada antes de esos cambios. Correcto.
- Regresión de instancias: una apertura antigua no sustituye la ruta de una posterior y sus guardados conservan los registros del otro archivo. Las selecciones nuevas no son reemplazadas por elecciones con fecha anterior. Correcto.
- Wolf Warrior.iso: MKV previo → ISO → guardado → nueva sesión. Playlist 0, búsqueda a 123,5 s, elección de audio y subtítulos desactivados se recuperan en 2D y MVC. Posiciones observadas: 123,498367 y 123,500000 s. Abrir una ISO inexistente no sustituye la última válida. Correcto.
- 28 comprobaciones de paquetes/recuperación y 8 de consulta correctas tanto en PowerShell 7.6.5 como en Windows PowerShell 5.1.26100.9711. Incluyen límites de peticiones, fecha HTTP, cabeceras malformadas, 404, límites de tamaño, manipulación del ZIP, recuperación con DLL bloqueada y uniones NTFS.
- Consulta HTTPS real con el auxiliar de Windows PowerShell 5.1: HTTP 404, resultado estructurado no-release. El manifiesto todavía no está publicado; no se confunde con un error de red.
- La política del manifiesto se valida con datos controlados: rechaza formato incompleto, versión/formato incompatibles, URLs ajenas o ambiguas y paquetes sin hash/tamaño válidos.
- Configuración personal: los hashes del contenido de config coinciden antes y después de las verificaciones finales. Todas las pruebas de guardado utilizan carpetas temporales aisladas. Las fuentes originales de vídeo, audio e ISO no se modifican.

Registros: verificacion/0.2.18/. El empaquetado valida cada archivo del portátil y genera SHA-256; las fuentes correspondientes y sus dependencias se verifican por CRC y hash.

## Límites y publicación

No se afirma haber reproducido todas las secuencias del fallo intermitente del usuario: se han corregido los mecanismos identificados y se han probado las regresiones descritas. No se ha reproducido completa la película ni se han certificado todos los discos o dispositivos.

Para habilitar la distribución pública de esta versión hay que subir el ZIP portátil y Enigmax-CinePlayer-update.json a la release estable v0.2.18. La actualización completa de una instalación desde Internet queda pendiente hasta esa publicación; las comprobaciones locales de extracción, sustitución y recuperación sí se han ejecutado. No se ha publicado nada en GitHub durante esta tarea. Las DLL y dependencias de reproducción son las mismas de 0.2.17.
