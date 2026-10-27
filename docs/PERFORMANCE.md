# Rendimiento: primera revisión de 0.4.1

Fecha: 27 de octubre de 2026. Base de código: `b043142` (atajo F6), posterior a la publicación pública 0.4.0. Esta revisión combina el arreglo de la barra solicitado con cambios pequeños y verificables; no cambia motores, dependencias, presupuestos de memoria ni formatos admitidos.

## Cambios conservados

| Zona | Trabajo eliminado o error corregido | Alcance |
| --- | --- | --- |
| Barra (`src/ui/chrome.rs`) | El scroll horizontal ocupa el ancho real de las pestañas, dejando el espacio libre para `StartDrag` | Mover la ventana no inicia una separación de pestaña. Los controles comparten el centro vertical |
| Ayuda de pestañas | La ruta completa se formatea dentro del callback de hover | Evita construir esa cadena por pestaña y frame cuando no se muestra la ayuda |
| Título nativo (`src/ui/mod.rs`) | Comparación de identidad del documento antes de preparar el texto | Evita asignar el nombre y título en cada llamada de lógica sin cambios. Actualiza al cambiar de libro o volver a Inicio |
| Portadas (`crates/runtime/src/covers.rs`) | El encoder PNG toma prestado el RGBA del render | Evita una copia temporal de `ancho × alto × 4` bytes por portada guardada; conserva el PNG y la escritura atómica |
| Instrumentación (`examples/performance_probe.rs`) | El cierre del intervalo de reposo se atiende también en lógica | En egui 0.36 un temporizador puede ejecutar lógica sin otro frame UI. La sonda ya no debe depender de pintar para terminar |

No se interpreta el ahorro de asignaciones como un porcentaje de mejora de FPS, apertura o RSS total.

## Experimento descartado

Se midió la caché real con `crates/runtime/examples/cache_bench.rs`: claves `RenderKey`, 8/32/128 entradas, 40 000 consultas por muestra, una ronda de calentamiento y nueve muestras. El candidato recorría las entradas mutables y actualizaba su edad directamente, evitando clonar la clave y repetir el hash.

Las primeras medianas mejoraron con ocho entradas, pero con 128 pasaron aproximadamente de 220 a 424 ns en `most_recent` y de 232 a 476 ns en `best_match`. Se descartó el candidato y `cache.rs` conserva el algoritmo de la base. Procesos distintos tienen semillas de HashMap distintas; esta prueba exploratoria no establece una distribución global ni un p95. Los resultados sólo justifican no adoptar un cambio con una señal de regresión en el caso denso.

Para repetir con el algoritmo productivo:

```powershell
cargo run -p crabpdf-runtime --release --example cache_bench -- artifacts/cache-current.json
```

El banco usa únicamente CPU, no abre documentos ni mide GPU, interfaz o importación.

## Revisión y siguientes mediciones

| Módulo | Situación observada | Siguiente trabajo, condicionado a medir |
| --- | --- | --- |
| Core / layout | La UI conserva el layout en un `Arc` y lo reconstruye cuando cambia su especificación | Medir documentos con miles de páginas y el coste de localizar páginas visibles antes de añadir índices |
| Application / selección y búsqueda | Las reglas no hacen I/O; seleccionar o extender un rango no copia el contenido de la página | Medir páginas con muchas líneas/glifos antes de cambiar la búsqueda de palabras o geometría |
| UI / movimiento | Render previo y geometría se reutilizan; movimiento continuo y marcas tienen recorridos de regresión nativos | Separar coste de hit testing, envío a GPU y presentación. Evitar sondeo permanente o animaciones adicionales para ocultar latencia |
| Runtime / render y caché | PDFium tiene un único propietario; se mantienen límites CPU/GPU/texto y canales acotados | Registrar aciertos, fallbacks y expulsiones con corpus real. El experimento anterior no se aplica |
| Importadores | La importación ya está separada del render; se preservan cancelación, límites y caché verificada | Comparar caché vacía/caliente por formato, incluyendo CPU/RSS del conversor hijo |
| Perfil y almacenamiento | `ProfileSession` prepara el guardado fuera de la UI; escritura atómica y reconciliación entre ventanas | Medir perfiles grandes y sesiones concurrentes antes de cambiar la política de guardado |
| Desktop | Diálogos y arranque quedan en adaptadores; el título ya usa identidad | Medir arranque por fases, resolución de herramientas y descubrimiento nativo en procesos nuevos |
| Distribución | Siguen los mismos motores, logo, licencias y características | Comparar ejecutable, DLL, conversores, ZIP e instalador por separado. No quitar formatos ni verificaciones para bajar tamaño |

## Referencia local Windows x64

Cinco procesos release en Intel Core i5-9400, escala 100 %, con PDF sintético de 24 páginas y EPUB de prueba. Disponibilidad observada por la sonda:

| Operación | Mediana | Mínimo–máximo |
| --- | ---: | ---: |
| Apertura PDF desde la app | 106.2 ms | 93.5–144.9 ms |
| Arranque de eframe hasta página disponible | 428.2 ms | 386.9–480.3 ms |
| Página siguiente precargada | 20.0 ms | 15.5–27.6 ms |
| Última página sin precargar | 46.5 ms | 31.4–75.1 ms |
| Desplazamiento, incluyendo estabilidad de 80 ms | 350.5 ms | 322.6–382.8 ms |
| Reabrir PDF en el mismo proceso | 81.4 ms | 69.0–190.9 ms |
| EPUB con caché de importación vacía | 316.5 ms | 152.4–464.4 ms |

El pico de working set del lector varió de 117.6 a 124.4 MiB (mediana 118.6 MiB). En cuatro intervalos de reposo no se contabilizó CPU y en el primero se midió 3.97 % de un núcleo; los intervalos duraron entre 3.88 y 3.98 segundos. La resolución del contador y la duración impiden interpretar esto como consumo permanentemente nulo.

El ejecutable x64 pasó de 11 400 192 a 11 366 400 bytes (−33 792 bytes, −0.30 %). El ZIP completo ocupa 43 597 347 bytes y el instalador 31 909 189 bytes; incluyen motores y licencias. Sus tamaños no equivalen al ejecutable aislado.

## Evidencia y límites

El registro versionado de esta entrega está en [releases/v0.4.1-validation.json](releases/v0.4.1-validation.json). Los logs, capturas y mediciones detalladas quedan en `artifacts/performance-chrome/`, ignorado por Git. La sonda se ejecuta sin compilación ni recorridos gráficos simultáneos, con datos de prueba propios.

La medición de apertura/navegación es disponibilidad después de enviar UI, no tiempo de presentación física. El observador sólo mide el proceso lector; no GPU ni recursos de conversores. Se registran min/mediana/max con un corpus sintético; la caché del sistema operativo no se controla. No hay una referencia de latencia equivalente anterior al arreglo de la sonda: una medición actual sirve de referencia para futuros cambios, no demuestra una aceleración respecto a 0.4.0. A00 y la aceptación cuantitativa completa de A07 siguen abiertas.
