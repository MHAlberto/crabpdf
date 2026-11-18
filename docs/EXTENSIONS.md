# Extensiones de CrabPDF 0.5.0

La 0.5.0 en desarrollo incorpora paquetes nativos opcionales y el SDK Rust `crabpdf-extension-api` 0.1.0. El protocolo inicial tiene `api_version = 1`. La aplicación descubre manifiestos y ejecuta un comando cuando el usuario lo solicita.

## Instalar y usar

1. Abre **Extensiones → Extensiones…**.
2. Pulsa **Instalar desde carpeta…** y elige la carpeta que contiene `extension.toml`.
3. Revisa el nombre, versión y comandos. Pulsa **Instalar paquete** y después **Activar**.
4. Pulsa **Seleccionar área del PDF…** y arrastra un rectángulo sobre una ecuación. También puedes seleccionar texto de una página y elegir **Texto → Enviar selección a extensión**.
5. Abre el comando del paquete. Su resultado aparece en la misma ventana y puedes editarlo o pulsar **Copiar resultado**.

En pantalla completa encontrarás Extensiones en la barra flotante del borde superior. Escape cancela la selección del área.

Los paquetes se guardan en `extensions/` dentro del directorio de datos del perfil. Cada versión tiene su propia carpeta. Activar otra versión cambia el registro de ese paquete de forma atómica. La desinstalación retira la versión elegida. Los paquetes instalados empiezan desactivados.

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

`input` admite `region` y `text`. `output` admite `text` y `latex`. El lector verifica la versión de API, la compatibilidad del host, la plataforma, las rutas y los identificadores de los comandos antes de instalar.

Las plataformas usan triples Rust como `x86_64-pc-windows-msvc`, `i686-pc-windows-msvc`, `x86_64-unknown-linux-gnu` y `aarch64-apple-darwin`. Cada paquete contiene el ejecutable apropiado para su plataforma.

## SDK Rust

El crate está en [crates/extension-api](../crates/extension-api/). Un proyecto externo puede depender del paquete `crabpdf-extension-api` mediante Git fijando una revisión de CrabPDF que contenga la API 1. En el propio repositorio se puede usar una dependencia por ruta. El SDK se versiona independientemente de la aplicación.

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

Cada ejecución recibe dos argumentos con rutas absolutas de intercambio.

```text
--crabpdf-request <request.json> --crabpdf-response <response.json>
```

El host escribe una solicitud JSON con API, identificador, comando, página desde 1 y región normalizada. El origen de la región es la esquina superior izquierda de la página sin el giro elegido en el visor. Se entrega el texto de esa área y, para un comando `region`, un PNG de hasta 2048 × 2048. Los permisos de copia del PDF se comprueban antes de producir esos datos.

El SDK lee la solicitud y escribe una respuesta con el mismo identificador. El resultado contiene el formato declarado y el texto, o un error. El host comprueba el identificador, API, formato y tamaño. La lectura y la escritura JSON están limitadas a 1 MiB. Cada proceso dispone de 120 segundos y se puede cancelar desde la interfaz. El proceso termina antes de liberar el paquete y limpiar los archivos de intercambio.

Las operaciones de instalación y activación se ejecutan en segundo plano y usan un bloqueo entre procesos. La instalación copia primero a una carpeta temporal y publica la versión mediante renombrado al terminar. Hay límites de 2 GiB, 4096 entradas y 16 niveles de carpetas. Se rechazan enlaces y archivos especiales.

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
