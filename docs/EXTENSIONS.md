# Extensiones de CrabPDF

CrabPDF 0.5 admite paquetes nativos opcionales y el SDK Rust `crabpdf-extension-api`. El SDK 0.2.0 añade los tipos del catálogo web. El protocolo inicial tiene `api_version = 1`. La aplicación descubre manifiestos y ejecuta un comando cuando el usuario lo solicita.

## Instalar y usar

1. Abre **Extensiones → Gestor de extensiones…**.
2. Pulsa **Desde carpeta…** y elige la carpeta que contiene `extension.toml`.
3. Revisa el nombre, versión y comandos. Pulsa **Instalar paquete** y después **Activar**.
4. Pulsa **Abrir herramienta → Seleccionar área…** y arrastra un rectángulo sobre una ecuación. También puedes seleccionar texto de una página y elegir **Texto → Enviar selección a extensión**.
5. Abre el comando del paquete. Su resultado aparece en la misma ventana y puedes editarlo o pulsar **Copiar resultado**.

En pantalla completa encontrarás Extensiones en la barra flotante del borde superior. Escape cancela la selección del área.

Los paquetes se guardan en `extensions/` dentro del directorio de datos del perfil. Cada versión tiene su propia carpeta. Activar otra versión cambia el registro de ese paquete de forma atómica. La desinstalación retira la versión elegida. Los paquetes instalados empiezan desactivados. Desde **Instaladas** puedes buscar actualizaciones, consultar las versiones y gestionar los atajos. Para retirar un paquete, abre **Detalles del paquete → Desinstalar…** y confirma la versión mostrada.

## OCR de ecuaciones y Pomodoro

Desde la 0.5.5, busca **100002** para OCR o **100003** para Pomodoro en **Extensiones → Gestor de extensiones → Catálogo**. Confirma la descarga y pulsa **Activar**. Cada paquete activo aparece con su nombre en **Extensiones**. En su ventana, abre **Atajos de teclado** para asignar con Alt+Mayús una tecla a **Abrir** y otra a **Cerrar**.

- **OCR.** En Windows x64 o Linux x64, pulsa **Descargar modelo (179 MB)**. Luego selecciona el área que contiene una ecuación y pulsa **Reconocer como LaTeX**. RapidLaTeXOCR procesa la imagen localmente. El OCR guarda la verificación SHA-256 del modelo y recalcula los hashes cuando cambian los archivos. El panel permite consultar el espacio y eliminar el modelo. Los pesos se conservan al actualizar o desinstalar el paquete. Si ya lo desinstalaste, reinstálalo y usa **Eliminar modelo** para liberar ese espacio. [Fuentes del modelo](https://github.com/RapidAI/RapidLaTeXOCR/releases/tag/v0.0.0).
- **Recuperación.** En **Instaladas**, los paquetes dañados aparecen con su diagnóstico y se pueden desinstalar antes de volver a descargarlos. Si falla el registro de activación, pulsa **Activar** en la versión que quieras recuperar.
- **Pomodoro.** Pulsa **Iniciar** en el panel sobre el PDF. Puedes pausar o reiniciar la sesión. Al terminar, el lector muestra un aviso y solicita atención al sistema. El reloj sigue en marcha al cerrar su ventana.

## OCR de texto local

Instala y activa **OCR de texto** con el código **100004** en el catálogo. Para compilar el paquete localmente, ejecuta **Build-ExtensionExample.ps1 -TextOcr** e instálalo desde **artifacts/extensions/text-ocr**. Usa **Usar página actual** o elige un área, ejecuta el reconocimiento en español o inglés y guarda el resultado en el cuaderno para buscarlo después. Requiere Tesseract con el idioma instalado. Si está en otra ubicación, configura **CRABPDF_TESSERACT** con la ruta absoluta a su ejecutable. [Instalación de Tesseract](https://tesseract-ocr.github.io/tessdoc/Installation.html).

## Módulos WASI

Un manifiesto puede declarar el valor **wasi** en la propiedad **runtime** y un ejecutable **.wasm** compilado para **wasm32-wasip1**. Configura **CRABPDF_WASMTIME** con la ruta absoluta al CLI Wasmtime. El módulo recibe únicamente el directorio temporal de intercambio. El host configura límites de ejecución y memoria y desactiva la red del módulo. Los paquetes nativos siguen requiriendo confianza y el catálogo se mantiene curado. [Opciones WASI de Wasmtime](https://docs.wasmtime.dev/cli-options.html).

## Paneles integrados en 0.8.1

Las extensiones activas aparecen en **Herramientas** en la barra de lectura. Elige una para abrir el panel lateral. Puedes cambiar de herramienta arriba del panel y usar los mismos accesos en la barra flotante de pantalla completa. Los atajos de abrir y cerrar se conservan.

- **Pomodoro** muestra el tiempo restante en la barra. Abre el panel para iniciar, pausar, continuar o reiniciar la fase. El reloj sigue al cerrar el panel y el aviso se muestra en el lector al terminar.
- **OCR** comparte el panel lateral. El panel se oculta al seleccionar un área, reaparece con el recorte listo y permite copiar o guardar el resultado en el cuaderno.
- **Estadísticas de lectura** muestra el documento actual, la sesión y los totales históricos. Cuenta visitas al abrir o cambiar de página y mide el tiempo con el documento en primer plano. También registra los enfoques completados con Pomodoro cuando ambas extensiones están activas. Los totales se guardan en el perfil local. El detalle por documento corresponde a su pestaña durante la sesión.
- **Guía de lectura** pinta una franja que permanece a la misma altura del visor al desplazar el PDF. Puedes ajustar posición y altura en el panel. Cerrar el panel conserva la franja y desactivar el paquete la retira.

Los paquetes locales se preparan con `scripts/Build-ToolExtensions.ps1 -Target x86_64-pc-windows-msvc -Statistics` y `-ReadingGuide`. Los ZIP quedan en `artifacts/site-extensions` y sus carpetas listas para instalar quedan en `artifacts/extension-build`. Para otro sistema usa su target Rust.

El SDK 0.5 añade la propiedad opcional `panel`. El host reconoce `reading_statistics` y `reading_guide` con `executable = ""` y `commands = []`. El protocolo de ejecución se mantiene en API 1 y los manifiestos existentes siguen siendo compatibles.

## Instalar por código

Abre el [catálogo oficial](https://mhalberto.github.io/crabpdf/extensions/) y copia el código de seis cifras. Abre **Extensiones → Gestor de extensiones → Catálogo**, introduce el código en **Instalar por código** y pulsa **Consultar**. Revisa la ficha antes de pulsar **Descargar e instalar**. Luego pulsa **Activar**. El código `100001` instala el ejemplo **Información de la selección**.

La consulta y la descarga usan HTTPS, solo al solicitarlo. El catálogo no recibe documentos ni selecciones. CrabPDF verifica el tamaño y SHA-256 del ZIP y compara el manifiesto con la ficha. Si instalas una versión nueva, puedes mantener la anterior activa hasta que decidas cambiarla, siempre que ambas sean compatibles con tu versión de CrabPDF.

## Publicar en el catalogo

1. Compila un ZIP por plataforma. Coloca `extension.toml`, el ejecutable y los recursos en la raíz del ZIP o en sus subcarpetas. Usa letras ASCII, números, guiones, puntos y guiones bajos en las rutas. El ZIP puede ocupar hasta 512 MiB y expandirse a 2 GiB como máximo.
2. Propón la extensión y sus fuentes al repositorio de CrabPDF. Una vez revisado, el mantenedor adjunta los ZIP a una entrega de GitHub del proyecto. Los paquetes siguen pudiendo desarrollarse en repositorios independientes.
3. Agrega una entrada a [docs/site/extensions.json](site/extensions.json) con un código de seis cifras único y estable. Copia del manifiesto `id`, `version`, `name`, `description`, `api_version` y `host_version`. En `assets` agrega por plataforma `target`, `release_tag`, `file` (el nombre ZIP), `bytes` y `sha256`. Si actualizas un paquete, conserva su código y publica ZIP con nombres nuevos.
4. GitHub Actions genera las fichas de la web y el endpoint `/extensions/catalog-v1.json` a partir del mismo catálogo. El SDK 0.2.0 expone el esquema en `crabpdf_extension_api::catalog`. El protocolo de ejecución mantiene su versión 1.

Los paquetes del catálogo público se compilan por plataforma y se alojan en GitHub Releases. Pages publica el catálogo y las fichas con enlaces a cada ZIP. Para una vista previa local con el ZIP de tu plataforma, ejecuta `./scripts/Build-ExtensionExample.ps1 -Archive`. Desde `docs/`, usa `CRABPDF_CATALOG_PREVIEW=1` y `npm run build` con las dependencias de npm ya instaladas.


## Paquete independiente

La carpeta se puede distribuir como ZIP y extraer antes de instalar. Incluye el ejecutable Rust, sus recursos, modelos y licencias. Puede desarrollarse en otro repositorio. El lector incorpora el SDK y el protocolo, mientras que los modelos OCR pertenecen al paquete que los utilice.

```text
equation-ocr/
  extension.toml
  bin/
    equation-ocr.exe
  models/
    model.onnx
  LICENSE
```

El ejemplo siguiente describe la interfaz que puede implementar un paquete OCR.

```toml
api_version = 1
id = "org.example.equation-ocr"
name = "Ecuaciones a LaTeX"
version = "0.1.0"
host_version = ">=0.5.0, <0.6.0"
target = "x86_64-pc-windows-msvc"
executable = "bin/equation-ocr.exe"
description = "Reconoce ecuaciones en el área seleccionada."

[[commands]]
id = "recognize"
title = "Obtener LaTeX de la ecuación"
input = "region"
output = "latex"
```

`input` admite `region`, `text` y `none` para comandos independientes del PDF. `output` admite `text` y `latex`. El lector verifica la versión de API, la compatibilidad del host, la plataforma, las rutas y los identificadores de los comandos antes de instalar.

Las plataformas usan triples Rust como `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc`, `x86_64-unknown-linux-gnu` y `aarch64-apple-darwin`. Cada paquete contiene el ejecutable apropiado para su plataforma.

## SDK Rust

El crate está en [crates/extension-api](../crates/extension-api/). El SDK se versiona independientemente de la aplicación. En un proyecto externo, agrégalo a tu `Cargo.toml` con la etiqueta de CrabPDF que incluye el SDK 0.3.0.

```toml
[dependencies]
crabpdf-extension-api = { git = "https://github.com/MHAlberto/crabpdf", tag = "v0.5.5" }
```

```rust
use crabpdf_extension_api::{serve, ExtensionOutput, OutputKind};

fn main() -> std::io::Result<()> {
    serve(|request| {
        let image = request.image.ok_or("Selecciona un área de la página")?;
        let latex = recognize_equation(&image.png_path)?;
        Ok(ExtensionOutput {
            kind: OutputKind::Latex,
            text: latex,
        })
    })
}
```

El autor del paquete implementa `recognize_equation` con su modelo. Los tipos `Manifest`, `ExtensionCommand`, `Request`, `SelectionImage`, `Response` y `ExtensionOutput` definen el contrato completo en Rust.

## Comunicación y datos

El host expone `CRABPDF_EXTENSION_DATA` para guardar datos entre ejecuciones. Cuando esté disponible `CRABPDF_EXTENSION_SCRATCH`, úsala para archivos temporales. Está en el mismo volumen y se limpia al terminar o cancelar el proceso.

Cada ejecución recibe dos argumentos con rutas absolutas de intercambio.

```text
--crabpdf-request <request.json> --crabpdf-response <response.json>
```

El host escribe una solicitud JSON con API, identificador, comando, página desde 1 y región normalizada. El origen de la región es la esquina superior izquierda de la página sin el giro elegido en el visor. Se entrega el texto de esa área y, para un comando `region`, un PNG de hasta 2048 × 2048 con el giro que mostraba el visor al seleccionar. Los permisos de copia del PDF se comprueban antes de producir esos datos.

El SDK lee la solicitud y escribe una respuesta con el mismo identificador. El resultado contiene el formato declarado y el texto, o un error. El host comprueba el identificador, API, formato y tamaño. La lectura y la escritura JSON están limitadas a 1 MiB. Los comandos con selección disponen de 120 segundos y los comandos `none` disponen de 600 segundos. Ambos se pueden cancelar desde la interfaz. El proceso termina antes de liberar el paquete y limpiar los archivos de intercambio.

Las operaciones de instalación y activación se ejecutan en segundo plano y usan un bloqueo entre procesos. La instalación copia primero a una carpeta temporal y publica la versión mediante renombrado al terminar. Cada paquete admite hasta 2 GiB, 4096 entradas y 16 niveles de carpetas. El perfil admite 64 versiones por extensión y hasta 256 versiones instaladas en total. Se rechazan enlaces y archivos especiales.

Los paquetes nativos se ejecutan con los permisos de la cuenta del sistema. Las entradas declaradas controlan lo que CrabPDF entrega mediante su API. El proceso nativo conserva las facultades que le conceda el sistema operativo, incluida la red. La primera implementación admite paquetes locales de confianza. Un backend WebAssembly con capacidades restringidas puede añadirse usando el mismo contrato de comandos y resultados.

## Ejemplo que se puede probar

```powershell
./scripts/Build-ExtensionExample.ps1
```

El script produce `artifacts/extensions/selection-info`. Instala esa carpeta desde la interfaz y ejecuta **Examinar el área seleccionada**. El ejemplo informa el tamaño de la imagen y el texto recibido. El motor de reconocimiento matemático se desarrollará en el paquete OCR.

La prueba de protocolo usa otro ejecutable deliberadamente limitado a fixtures, con respuestas inválidas y cancelación.

```powershell
cargo build --locked --offline -p crabpdf-extension-api --example protocol_fixture
$env:CRABPDF_TEST_EXTENSION = (Resolve-Path target/debug/examples/protocol_fixture.exe).Path
cargo test --locked --offline --test extensions -- --ignored --test-threads=1
```

Las pruebas nativas de captura se ejecutan con PDFium mediante `tests/native_pdf.rs`.
