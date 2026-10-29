# Actualizaciones portátiles de Enigmax CinePlayer 0.2.19

Al arrancar y desde Acerca de > Buscar actualizaciones, un hilo secundario consulta un manifiesto público alojado como asset de GitHub Releases, sin consumir la cuota de la API REST. La reproducción no espera a la red. APP_VERSION procede de Cargo.toml. Los modos --probe no consultan actualizaciones. Una exclusión entre procesos evita consultas simultáneas.

Endpoint: https://github.com/Enigmaxin/Enigmax-CinePlayer/releases/latest/download/Enigmax-CinePlayer-update.json
User-Agent: Enigmax-CinePlayer-Updater/<versión>

Se utiliza SemVer: se ignoran versiones anteriores, iguales, preliminares y borradores. El único asset aceptado es Enigmax-CinePlayer-Portable-MAJOR.MINOR.PATCH-windows-x64.zip, con tamaño entre 4 KiB y 512 MiB y digest SHA-256 publicado en el manifiesto. No se descarga ni ejecuta un instalador. No se registra la aplicación ni se cambian asociaciones o accesos directos.

Tras la aceptación se descarga el ZIP en una carpeta propia de TEMP. Windows PowerShell 5.1, .NET y Windows Forms, incluidos en Windows 11, ejecutan el auxiliar incrustado. No hay servicios residentes ni bibliotecas extraídas para la reproducción. El reproductor carga sus DLL desde su carpeta permanente. El auxiliar usa TLS y validación normal de certificados; acepta solo HTTPS y hosts previstos de GitHub, con un máximo de cinco redirecciones. La ventana atiende Cancelar mientras espera conexión o datos, con límites de conexión y lectura. La verificación y extracción pueden ocupar brevemente esa ventana.

Antes de cerrar se verifica el tamaño y SHA-256 del ZIP, su manifiesto package.manifest.json, los hashes y tamaños de todos sus archivos y la versión y arquitectura PE del EXE. Se rechazan rutas arbitrarias, duplicados sin distinguir mayúsculas, enlaces, uniones NTFS, archivos de configuración y expansiones mayores de 1 GiB. Hay un máximo de 256 archivos de contenido. El ZIP tiene sus archivos directamente en la raíz; no contiene una carpeta contenedora ni entradas de directorios.

La preparación se hace en una carpeta .enigmax-update-<GUID> dentro de la carpeta de la aplicación, en el mismo volumen. Debe haber permisos y espacio para el paquete nuevo y la recuperación temporal. Un bloqueo exclusivo impide dos sustituciones simultáneas. El auxiliar captura un handle del proceso antes de descargar y comprueba su identidad. Tras validar, señala ready; la aplicación termina y el auxiliar espera a que el proceso desaparezca, hasta 60 segundos. Vuelve a validar y sustituye el EXE y sus bibliotecas como conjunto. Retira únicamente archivos antiguos registrados en el manifiesto anterior y reinicia sin abrir la última película.

Si falla una sustitución se intenta restaurar los archivos anteriores. La recuperación se ha probado bloqueando una DLL de destino. Al terminar correctamente se eliminan la preparación y los archivos temporales propios. Si la recuperación no puede completarse, se conserva la carpeta de recuperación y el error indica su ruta. No hay garantía de transacción atómica ante un corte eléctrico o finalización forzada del auxiliar durante una sustitución de varios archivos. Cierra las otras instancias antes de actualizar. No se borran vídeos ni subtítulos del usuario y config/ se conserva.

Los errores de consulta se registran en config/updater-status.log sin diálogos durante el arranque. La comprobación manual sí informa del fallo. GitHub puede devolver 403/429, faltar una release o impedir una descarga. No se reintenta en bucle. config/updater-cache.json conserva una espera de 12 horas entre consultas automáticas correctas o sin release, 15 minutos ante fallos y al menos 60 segundos entre consultas manuales. Los límites HTTP 403/429 respetan Retry-After y X-RateLimit-Reset también entre reinicios y peticiones manuales. Un 404 se informa como ausencia de una versión con manifiesto; no es un error genérico. El registro y la consulta manual muestran el estado, la espera y el enlace de descargas. El SHA-256 demuestra coincidencia con el asset publicado; no sustituye la firma digital. Las políticas corporativas que prohíban PowerShell pueden impedir actualizar. No se cambia la política global del equipo.

## Crear y publicar un paquete

1. Ejecutar build.ps1 y revisar las pruebas.
2. Ejecutar build-portable.ps1. Comprueba que Cargo.toml y el EXE coinciden, prepara el manifiesto, genera el ZIP y verifica de nuevo su contenido extraído. Emite también un archivo .sha256 y Enigmax-CinePlayer-update.json junto al ZIP, con versión, URL exacta, tamaño y SHA-256. Este manifiesto no forma parte del ZIP instalado.
3. Cuando el usuario autorice publicar, subir el código al repositorio y adjuntar el ZIP y Enigmax-CinePlayer-update.json a una release pública estable v0.2.19. El nombre exacto del ZIP será Enigmax-CinePlayer-Portable-0.2.19-windows-x64.zip. Los archivos automáticos Source code de GitHub no sustituyen este paquete.
4. Comprobar que la URL pública del manifiesto devuelve el documento correcto y que una versión anterior detecta la nueva. Las versiones antiguas consultan la API; 0.2.18 y posteriores consultan el manifiesto. Esta prueba real de distribución queda pendiente hasta publicar un ZIP compatible. No se publican releases automáticamente.

build-installer.ps1 e installer/ permanecen únicamente como fuentes históricas; no intervienen en la compilación portátil ni se han generado instaladores nuevos.
Las versiones antiguas cuyo actualizador solo admite Setup necesitan descargar y extraer manualmente el primer ZIP portátil. Después podrán usar el nuevo mecanismo.

La primera migración desde 0.2.7 a este nuevo motor debe hacerse descargando el ZIP completo: su validador anterior no admite todos los nombres de DLL nuevos. Las versiones 0.2.8 en adelante admiten el conjunto compartido.
