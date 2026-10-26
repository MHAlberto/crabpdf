# CrabPDF 0.4.0

Fecha: 26 de octubre de 2026. Cierre de la serie de mejoras posterior a 0.3.6, con paquetes Windows x64/x86 y Ubuntu/Linux x64. La versión 1.0 conserva sus criterios del [plan](V1_PLAN.md).

## Alcance

Esta entrega reúne las [correcciones de interacción](INTERACTION_FIXES.md), la extracción modular [A01/A02](architecture/PHASE_1.md), [A02b](architecture/PHASE_2.md), [A02c](architecture/PHASE_3.md), [A03a](architecture/PHASE_4.md) y [A03b](architecture/PHASE_5.md), y el [instalador de Windows](WINDOWS_INSTALLER.md).

No cambia las versiones de dependencias ni los esquemas de perfiles y marcas. La actualización conserva la lectura guardada y el comportamiento de importación. Las cachés mantienen sus presupuestos y el motor nativo sigue teniendo un único propietario.

Se corrige una discrepancia de empaquetado en la CI x86: PDFium se prepara en `native/win-x86` y su ruta absoluta se entrega a las pruebas. Antes se descargaba en `native`, lo que permitía ejecutar los contratos pero impedía encontrar la DLL al generar el instalador.

Los paquetes de fuentes de formatos pasan a tener la versión de la entrega e incluyen el script Linux y la referencia correcta al repositorio. Los archivos fuente originales y sus hashes se conservan.

## Distribución

Los instaladores Windows llevan el logo original, licencia, componentes opcionales, selección de carpeta, progreso y finalización. Se instalan por usuario. Las opciones de escritorio y «Abrir con» son opcionales; no se cambia el lector PDF predeterminado.

Los paquetes Ubuntu/Linux incluyen el lector actualizado, PDFium y los conversores propios. El DEB usa APT para DjVuLibre/7-Zip y las bibliotecas declaradas. El tar requiere esas herramientas del sistema. La base de compilación Linux se mantiene en Debian Bookworm con glibc 2.36.

La entrega sigue sin firma digital porque aún no existe certificado. Los hashes acreditan coincidencia con los archivos publicados; no sustituyen Authenticode ni una certificación independiente. La firma real quedó preparada para cuando haya una identidad válida.

## Validación de cierre

Registro en preparación en `artifacts/release-0.4.0/`. La publicación requiere verificar los paquetes definitivos y la CI de la revisión etiquetada. El informe final se registra en `docs/releases/v0.4.0-validation.json`.

Se conservarán por separado pruebas de motor, regresión gráfica, instalación/actualización/desinstalación, manifiestos, arquitecturas y limitaciones. Una compilación o captura de egui no se presentará como prueba de un escritorio físico.

## Pendientes

A03c y la extracción de crates A04/A05 continúan después de este cierre. Siguen abiertos la medición comparable de A00, equidad de búsqueda bajo carga sostenida, ciclo de vida completo del hilo nativo y la matriz de paridad 1.0.

No se distribuye un binario macOS en esta entrega. La revisión interactiva/DPI de todas las páginas del instalador, escritorios físicos Linux, Wayland, Windows de 32 bits nativo y Windows 11 requieren sus recorridos propios.
