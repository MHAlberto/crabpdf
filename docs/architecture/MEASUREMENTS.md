# Mediciones de la migración de arquitectura

El instrumento `examples/performance_probe.rs` observa disponibilidad del raster tras enviar la UI. No mide cuándo el compositor presenta físicamente el frame. La aplicación utiliza egui/eframe, PDFium y sus cachés reales; el instrumento no reemplaza el motor.

## Reproducción en Windows

Preparar PDFium, herramientas y muestras según [README](../../README.md), y compilar sin procesos de medición activos:

```powershell
cargo build --locked --offline --release --target x86_64-pc-windows-msvc --bin crabpdf --example performance_probe
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Measure-Architecture.ps1 -Output artifacts/architecture/unique-output -Runs 5
```

La salida debe ser nueva. El observador genera un PDF original determinista de 24 páginas, usa el EPUB del corpus de pruebas y abre cinco procesos independientes. Cada proceso recibe un perfil y un directorio de datos propios. El tiempo de compilación no se incluye en las latencias. El arranque se mide desde antes de `eframe::run_native`; no incluye la creación del proceso ni la preparación previa del instrumento.

Se registran apertura PDF, arranque hasta disponibilidad, navegación a página precargada y a la última página sin precargar, desplazamiento, reapertura en el mismo proceso y EPUB con caché interna vacía. El desplazamiento incluye una puerta de estabilidad de 80 ms; no debe interpretarse como tiempo puro de render. El observador recoge pico de working set del proceso de la app y CPU durante cuatro segundos de reposo. CPU se expresa como porcentaje de un núcleo, sin dividir entre procesadores lógicos.

El informe conserva min/mediana/max, muestras individuales, versión de herramientas, viewport/escala, hash del ejecutable, PDFium, corpus, manifiesto de conversores y fuentes Rust. El estado de Git permite identificar los cambios de instrumentación presentes al medir. La revisión base y los hashes de fuentes distinguen la app anterior de la app migrada aunque ambas conserven la versión de desarrollo 0.3.6.

## Límites

- La caché del sistema operativo no se vacía: una apertura en proceso nuevo no equivale a disco frío.
- El instrumento pide frames cada 8 ms durante operaciones activas; la resolución y la carga introducida pertenecen al método. En reposo sólo agenda el final del intervalo.
- El working set corresponde a la app, incluyendo sus bibliotecas cargadas; no suma procesos conversores ni mide asignación GPU.
- Las muestras sintéticas no sustituyen un corpus grande ni la comparación con SumatraPDF.
- Cinco repeticiones permiten mostrar dispersión básica, no estimar p95 con precisión.
- Windows x64 no constituye evidencia de rendimiento en x86, Linux o macOS.
- Los umbrales provisionales del plan sirven para investigar regresiones; no convierten variaciones pequeñas de una máquina de uso normal en una mejora demostrada.

Las primeras muestras anteriores a la extracción están en [evidence/a00-before-windows-x64.json](evidence/a00-before-windows-x64.json). La [comparación de las cuatro series](evidence/a02-measurements-windows-x64.json) y su [interpretación](PHASE_1.md) incluyen el control con el binario original, identificado por SHA-256. En esa ejecución de control, el checkout del observador ya contenía el código nuevo: su procedencia de aplicación se toma de la evidencia inicial y del hash del instrumento, no de las fuentes presentes al ejecutar el observador. El ejecutable principal se registra para tamaño; el código de aplicación medido está enlazado dentro del instrumento. Las mediciones adicionales y la cobertura de recursos/plataformas pendientes mantienen A00 abierto como hito completo.
