# CrabPDF 0.4.0

Fecha: 26 de octubre de 2026. Cierre de la serie de mejoras posterior a 0.3.6, con paquetes Windows x64/x86 y Ubuntu/Linux x64. La versión 1.0 conserva sus criterios del [plan](V1_PLAN.md).

## Alcance

Esta entrega reúne las [correcciones de interacción](INTERACTION_FIXES.md), la extracción modular [A01/A02](architecture/PHASE_1.md), [A02b](architecture/PHASE_2.md), [A02c](architecture/PHASE_3.md), [A03a](architecture/PHASE_4.md) y [A03b](architecture/PHASE_5.md), y el [instalador de Windows](WINDOWS_INSTALLER.md).

No cambia las versiones de dependencias ni los esquemas de perfiles y marcas. La actualización conserva la lectura guardada y el comportamiento de importación. Las cachés mantienen sus presupuestos y el motor nativo sigue teniendo un único propietario.

Se corrige una discrepancia de empaquetado en la CI x86: PDFium se prepara en `native/win-x86` y su ruta absoluta se entrega a las pruebas. Antes se descargaba en `native`, lo que permitía ejecutar los contratos pero impedía encontrar la DLL al generar el instalador.

Los paquetes de fuentes de formatos pasan a tener la versión de la entrega e incluyen el script Linux y la referencia correcta al repositorio. Sus rutas ZIP usan separadores estándar para extracción en Windows y Linux. Los archivos fuente originales y sus hashes se conservan.

## Distribución

Los instaladores Windows llevan el logo original, licencia, componentes opcionales, selección de carpeta, progreso y finalización. Se instalan por usuario. Las opciones de escritorio y «Abrir con» son opcionales; no se cambia el lector PDF predeterminado.

Los paquetes Ubuntu/Linux incluyen el lector actualizado, PDFium y los conversores propios. El DEB usa APT para DjVuLibre/7-Zip y las bibliotecas declaradas. El tar requiere esas herramientas del sistema. La base de compilación Linux se mantiene en Debian Bookworm con glibc 2.36.

La entrega sigue sin firma digital porque aún no existe certificado. Los hashes acreditan coincidencia con los archivos publicados; no sustituyen Authenticode ni una certificación independiente. La firma real quedó preparada para cuando haya una identidad válida.

## Validación de cierre

Resultados locales y de la CI del candidato, registrados en [v0.4.0-validation.json](releases/v0.4.0-validation.json):

- Windows x64: formato, Clippy y 101 pruebas únicas, incluidas las nativas normalmente ignoradas. Los helpers de procesos hijos pertenecen a sus contratos.
- Instaladores x64/x86: instalación, 609 archivos del manifiesto, actualización, rechazo de carpetas ajenas/ejecutable bloqueado, desinstalación y conservación de un archivo ajeno.
- Actualización real del instalador x64 de 0.3.6 a 0.4.0: versión del ejecutable comprobada y archivo ajeno conservado.
- Portables Windows: 607 archivos verificados en cada ZIP. Fuentes de formatos: 13 archivos verificados también desde Linux, con rutas ZIP estándar.
- Debian Bookworm: compilación optimizada Linux con Rust 1.96.0. TAR y DEB: 408 hashes internos comprobados por paquete.
- Ubuntu 24.04: instalación del DEB, versión 0.4.0, 408 archivos instalados verificados, ocho pruebas PDF y siete de formatos/marcas con los conversores empaquetados.
- Interfaz real con entrada simulada: 32 capturas Windows y 68 Ubuntu durante selección, resaltado, subrayado, deshacer, desplazamiento y apertura de formatos. Además se inició el ejecutable instalado en Ubuntu y se capturó su ventana.
- La [CI final de e17c06d](https://github.com/MHAlberto/crabpdf/actions/runs/40384498542) aprobó Windows x64/x86, Ubuntu x64 y macOS Intel/ARM, incluidos los instaladores de ambas arquitecturas Windows.
- Se publicó [v0.4.0](https://github.com/MHAlberto/crabpdf/releases/tag/v0.4.0) y se comprobaron los diez archivos públicos contra su tamaño y SHA-256 locales.

Se revisaron las capturas de lectura del DEB instalado y de resaltado Windows. Los recorridos Ubuntu usan Xvfb, Mesa por software y Openbox para la ventana instalada. No equivalen a escritorios físicos ni a pruebas en Wayland. La revisión interactiva de las páginas/DPI del asistente sigue pendiente.

Los logs, capturas y ejecutables de prueba se conservan localmente en `artifacts/release-0.4.0/`. No se deduce una mejora global de latencia de esta campaña de regresión ni se incorporan los artefactos de pruebas a los instaladores.

## Pendientes

A03c y la extracción de crates A04/A05 continúan después de este cierre. Siguen abiertos la medición comparable de A00, equidad de búsqueda bajo carga sostenida, ciclo de vida completo del hilo nativo y la matriz de paridad 1.0.

No se distribuye un binario macOS en esta entrega. La revisión interactiva/DPI de todas las páginas del instalador, escritorios físicos Linux, Wayland, Windows de 32 bits nativo y Windows 11 requieren sus recorridos propios.
