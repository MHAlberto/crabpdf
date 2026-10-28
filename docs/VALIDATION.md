# Validación y límites

Este documento orienta sobre la evidencia vigente. Los comandos reproducibles están en [Desarrollo](DEVELOPMENT.md); los resultados históricos se conservan por separado.

## Preparación local de 0.4.2

El [registro de 0.4.2](releases/v0.4.2-validation.json) conserva la revisión de fuentes, hashes, compilaciones release x64/x86, clippy, regresiones y comparación de memoria. Pasaron 112 pruebas por arquitectura con los casos nativos habilitados en debug; el ajuste final de alineación tiene además regresiones propias y una [captura nativa x64](screenshots/properties-0.4.2.png). Los ZIP y las fuentes de conversores se verificaron con `Verify-Portable.ps1`.

El recorrido sintético mostró un pico residente mediano de 261.5 a 169.2 MiB, sin acreditar una aceleración global. Véanse el método y los límites en [Rendimiento](PERFORMANCE.md). Es una preparación local sin commit ni publicación; no hay validación nueva de Linux/macOS, instaladores o un SO Windows de 32 bits nativo.

## Entrega 0.4.1

| Evidencia | Qué acredita | Qué no acredita |
| --- | --- | --- |
| [CI de la revisión etiquetada](https://github.com/MHAlberto/crabpdf/actions/runs/40565789343) | Compilación y pruebas en Windows x64/x86, Ubuntu x64 y macOS Intel/ARM | Matriz gráfica física completa |
| [Registro de publicación](releases/v0.4.1-publication.json) | Procedencia de paquetes CI Windows, hashes, instalación/upgrade/uninstall y comprobación Ubuntu | Certificación de seguridad o firma digital |
| [Validación local](releases/v0.4.1-validation.json) | 105 pruebas locales, geometría de ventana, interacción y cinco mediciones de la compilación local | Que el binario local tenga el mismo hash que uno compilado en GitHub |
| [Rendimiento](PERFORMANCE.md) | Método, muestras, cambios conservados y experimento de caché descartado | Una mejora global porcentual frente a 0.4.0 |

Windows x86 se comprueba bajo WOW64. Ubuntu usa Docker, Mesa y Xvfb; la ventana instalada y F6 se comprobaron mediante captura directa. La captura del escritorio completo devolvía negro en ese entorno, por lo que no se utiliza como evidencia de representación correcta.

Windows 11 físico, Windows de 32 bits nativo, Wayland, otras distribuciones, macOS gráfico y tecnologías de asistencia siguen necesitando pruebas propias. La ausencia de un fallo en CI no cierra esas rutas.

## Tipos de comprobación

- **Bibliotecas y contratos:** reglas, geometría, protocolo, perfil y dirección de dependencias.
- **Motor nativo:** PDFium y conversores con documentos originales de tests/fixtures.
- **Interacción:** RawInput en una aplicación nativa para navegación, marcas, búsqueda y pestañas.
- **Ventana:** observación del cliente y monitor, además del framebuffer.
- **Distribución:** hashes, bibliotecas, licencias, instalación, actualización y desinstalación.
- **Documentación y web:** destinos locales versionados, generación de recursos, accesibilidad estructural y revisión en navegador.

Las pruebas marcadas como ignoradas requieren una ejecución explícita con sus dependencias. Las capturas añaden coste y no sirven por sí solas para medir latencia de presentación.

## Registrar una revisión

Guarda el commit, estado de fuentes, versión de herramientas, plataforma y comandos. Para paquetes, registra hashes y procedencia. Para medidas, conserva muestras, corpus y límites. Separa evidencia previa y posterior; no sumes ejecuciones solapadas como si fueran casos independientes.

Los logs, perfiles aislados y capturas de trabajo van en artifacts, fuera de Git. Los registros resumidos que sustentan una entrega permanecen en releases o en los directorios de evidencia correspondientes.

[Informes antiguos](history/README.md) · [Migración](architecture/PHASE_6.md) · [Pendientes 1.0](V1_PLAN.md)
