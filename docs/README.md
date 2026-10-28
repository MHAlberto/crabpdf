# Documentación de CrabPDF

La versión pública es **0.4.1**, de desarrollo. [Descargas](https://github.com/MHAlberto/crabpdf/releases/tag/v0.4.1) · [Web](https://mhalberto.github.io/crabpdf/) · [Repositorio](../README.md)

## Empezar

| Necesito… | Documento |
| --- | --- |
| Abrir documentos, marcar texto o consultar atajos | [Guía de uso](USAGE.md) |
| Saber qué extensiones funcionan | [Formatos y anotaciones](FORMATS_AND_ANNOTATIONS.md) |
| Compilar, probar o empaquetar | [Desarrollo](DEVELOPMENT.md) |
| Preparar una contribución | [CONTRIBUTING.md](../CONTRIBUTING.md) |
| Editar o desplegar la página | [Web](WEBSITE.md) |

## Mantener el proyecto

| Tema | Referencia |
| --- | --- |
| Paquetes y dirección de dependencias | [Arquitectura](ARCHITECTURE.md) |
| Motivos de la arquitectura | [ADR-0001](adr/0001-modular-architecture.md) |
| Medición y experimentos | [Rendimiento](PERFORMANCE.md) |
| Pruebas y alcance real de la evidencia | [Validación](VALIDATION.md) |
| Instalación y firma Windows | [Instalador](WINDOWS_INSTALLER.md) |
| Revisión y limpieza del repositorio | [Revisión de mantenimiento](REPOSITORY_REVIEW.md) |
| Componentes redistribuidos | [Avisos](THIRD_PARTY_NOTICES.md), [inventario](dependencies.csv) y [licencias originales](licenses-upstream/sources.json) |

## Objetivo 1.0

[Alcance del producto](ROADMAP.md), [plan por rutas](V1_PLAN.md) y [backlog](V1_BACKLOG.csv). El [inventario de paridad](parity/cases.md) y la [referencia fijada](parity/reference.json) permiten evaluar comportamientos concretos.

Una ruta completada de arquitectura no demuestra paridad de funciones. La impresión, la edición general, SyncTeX y la accesibilidad completa siguen pendientes. V2 contempla extensiones; no se presenta como disponible.

## Entregas y evidencia

- [Notas 0.4.1](releases/v0.4.1.md): funciones y paquetes publicados.
- [Publicación 0.4.1](releases/v0.4.1-publication.json): procedencia CI, hashes y comprobación Ubuntu.
- [Validación local 0.4.1](releases/v0.4.1-validation.json): compilación local y referencia de rendimiento anterior a publicar los binarios CI. Sus hashes y tamaños no describen necesariamente los ejecutables distribuidos por GitHub.
- [Migración modular](architecture/PHASE_6.md): extracción de paquetes, pruebas y límites cuantitativos.

Las notas de versiones anteriores permanecen en releases. Los [informes históricos](history/README.md) describen el código y los resultados de su fecha. Los logs de artifacts son archivos locales, no material descargable desde un clon limpio.
