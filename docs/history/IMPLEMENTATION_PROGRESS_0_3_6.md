# CrabPDF 0.3.6 — pantalla completa, favoritos y arrastre

> Documento histórico. Describe una revisión anterior. Consulta el [índice actual](../README.md) para las instrucciones vigentes.


**Fecha:** 23 de octubre de 2026. Avance parcial de R10, R13 y R30 sobre [0.3.5](IMPLEMENTATION_PROGRESS_0_3_5.md). No declara paridad 1.0.

## Correcciones

### Pantalla completa real

La captura del framebuffer no detectaba el recorte del cliente nativo. Desde una ventana maximizada, Windows mantenía un cliente de **1920×1040** dentro de un monitor de **1920×1080**. La ruta maximizada de `WM_NCCALCSIZE` en winit utiliza el área de trabajo, que excluye la barra de tareas. También se observó un origen desplazado un píxel en el marco sin decorar.

`ReaderApp::sync_fullscreen` centraliza F11, Escape, presentación y arranque CLI. En Windows retira temporalmente el estado maximizado antes de entrar, vuelve a aplicar decoraciones para actualizar la sombra y restaura la maximización al salir. Las preferencias de barra nativa se conservan. No se agrega FFI al lector ni se cambia de framework.

La regresión comprobó un cliente de **1920×1080, origen 0,0**, desde ventana normal y maximizada, con barra propia y nativa. Al salir se recupera la ventana maximizada cuando corresponde. Las medidas pertenecen al monitor utilizado; el código no fija esa resolución.

### Favoritos del libro abierto

- El panel filtra por la ruta de origen del documento activo y ordena por página.
- Permite guardar/quitar la página actual y eliminar entradas individuales.
- Elegir un favorito navega en la pestaña actual y conserva zoom, modo y giro.
- Los favoritos de otros libros permanecen guardados y aparecen al abrirlos.
- El formato TOML existente se mantiene; las etiquetas automáticas antiguas se muestran como página sin reescribirlas.
- Se corrige un exceso de ancho de dos píxeles en las filas que hacía crecer el panel entre frames.

Es una preferencia explícita del usuario, no una declaración de paridad de toda la gestión de favoritos de SumatraPDF.

### Arrastrar para duplicar

Arrastrar el nombre de una pestaña fuera de la barra y soltar crea otra pestaña en **la misma ventana**. Conserva el documento original, página, zoom y ancla de lectura; también funciona al arrastrar una pestaña inactiva. Se reutiliza la acción existente de duplicado.

Volver a la barra, perder el botón o pulsar Escape cancela el gesto. Los botones de cierre mantienen su acción y la zona vacía de la barra sigue moviendo la ventana. No se implementa separar una pestaña en otra ventana.

## Regresiones

`examples/document_chrome_smoke.rs` genera 25 capturas por recorrido con preferencias aisladas y PDFs de prueba propios. Comprueba dos libros con favoritos separados, navegación al 125 % sin cambiar el modo, guardar/quitar, duplicación de una pestaña inactiva, posición normalizada conservada, original intacto, cancelaciones, pantalla completa desde normal/maximizada y restauración.

`scripts/Test-WindowGeometry.ps1` ejecuta el ejemplo y observa solo su proceso hijo. Compara mediante Win32 el origen y tamaño del cliente con el monitor físico, cubriendo el espacio exterior que no aparece en una captura de egui. No modifica ventanas ajenas. El script general `Test-Local.ps1` incorpora ambos estilos de barra por arquitectura para futuras ejecuciones.

```powershell
cargo run --locked --example document_chrome_smoke -- artifacts/document-chrome
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Test-WindowGeometry.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Test-WindowGeometry.ps1 -Native -ReportRoot artifacts/document-chrome-native
```

En Linux el ejemplo se ejecuta con un gestor de ventanas: la prueba de maximización necesita que el entorno implemente esa operación.

## Validación y paquetes

Informe local independiente: `artifacts/validation-chrome-20261023/`. Se ejecutaron formato, Clippy con advertencias como errores y la suite ordinaria: 28 unitarias, cuatro de posición de lectura y la regresión de perfil entre procesos. Se añadió una prueba de filtrado/orden/eliminación entre libros que comparten el mismo nombre de archivo en distintas carpetas.

Los binarios mantienen Rust/egui/eframe, PDFium 7881, los conversores verificados de 0.3.5 y sus presupuestos de caché. No hay nuevas dependencias. Los conversores no se recompilan en esta entrega; sus fuentes siguen en `crabpdf-0.2.0-format-tools-sources.zip`.

Se verificaron los siguientes recorridos y entornos:

| Entorno | Resultado |
| --- | --- |
| Windows 10 x64, build 19045 | 25 escenas con barra propia y 25 con barra nativa; medidas cliente/monitor desde normal y maximizada. Siete pruebas PDF nativas; 46 escenas de refinamiento, transiciones normal/reducidas y lectura restaurada entre tres procesos. |
| Windows x86 bajo WOW64 | 25 escenas con cada barra, mediciones Win32 sincronizadas y siete pruebas con PDFium x86. |
| Ubuntu 24.04 x86_64 | Instalación real del DEB y sus dependencias, manifiestos instalado/DEB/tar, 28 unitarias, siete pruebas PDF, cinco de marcas PDF y exportación entre procesos. Openbox sobre Xvfb/Mesa: 25 escenas con cada barra, 25 del visor y recorrido de transiciones. El ejecutable instalado abrió un PDF durante ocho segundos, con el timeout esperado 124. |

Los programas de producción usan el perfil release habitual. Los ayudantes Linux se enlazaron sin LTO entre crates para reducir su tiempo de construcción, usando la misma biblioteca del lector. La prueba de geometría Windows sincroniza cada captura con la medición externa: no avanza hasta recibir la confirmación del observador. No se atribuye una ejecución integral a `Test-Local.ps1`; se usaron los comandos individuales del informe local.

La [CI del código de implementación](https://github.com/MHAlberto/crabpdf/actions/runs/39986835302), commit `4421085`, aprobó los cinco targets: Windows x64/x86, Ubuntu x64 y macOS Intel/ARM. Incluye Clippy, pruebas ordinarias, siete pruebas PDF por target, formatos/marcas en Windows y build release. El cierre de entrega añade documentación y sincronización del programa de pruebas, sin cambiar las fuentes del lector.

[Captura de favoritos por libro](../screenshots/favorites.png). La evidencia compacta de límites nativos se conserva en [mediciones Win32](../parity/evidence/v0.3.6-window-geometry.json).


## Límites

Windows x86 se comprueba bajo WOW64, no en un sistema operativo de 32 bits. Las medidas nativas corresponden a Windows 10 x64 build 19045 y a un monitor; faltan varios monitores, escalas y Windows 11. La ejecución Linux se realiza en Ubuntu 24.04 con Xvfb/Mesa y Openbox; no certifica escritorios físicos, Wayland u otras distribuciones.

La CI de macOS comprueba código y PDF, sin certificar interfaz ni aportar un paquete `.app`. No se repite localmente el corpus completo de conversores/Kindle. No se afirma que estas pruebas cubran cualquier PDF o controlador gráfico.

Siguen abiertos impresión, selección entre páginas, editor completo de anotaciones, formularios, firmas, integración y los restantes criterios 1.0.

## Paquetes verificados

Los ZIP contienen 607 archivos verificados por arquitectura; DEB y tar, 408 cada uno. Los ejecutables coinciden con los binarios de producción compilados.

| Paquete | Bytes | SHA-256 |
| --- | ---: | --- |
| `crabpdf-0.3.6-linux-x64.tar.gz` | 40598694 | `d996e399812e9c7ea40d898e6ddbc2832e0015ebb37d2c9573bc68ea34ce9c66` |
| `crabpdf-0.3.6-ubuntu24.04-amd64.deb` | 30614016 | `5d0e9def9aac5973f7edf78da48c483f1d4abe1b179d399855955f6aa41c0f50` |
| `crabpdf-0.3.6-win-x64.zip` | 43579681 | `dcd6e34e944ff2e45980211afbd70514d4c3825cfeb0d4ab513cf6f5f1cb3804` |
| `crabpdf-0.3.6-win-x86.zip` | 43232893 | `7283ba60425944d2f80f9aa5fd022e5914c7bf81e81701d9dd28931c61c7fca1` |
