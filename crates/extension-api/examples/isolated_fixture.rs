//! WASI capability fixture. The host supplies its own isolated sentinel path.
use crabpdf_extension_api::{ExtensionOutput, OutputKind, serve};

fn main() -> std::io::Result<()> {
    serve(|request| {
        let outside = request.text.ok_or("Falta el archivo de control.")?;
        if std::fs::read(outside).is_ok() {
            return Err("El módulo pudo leer fuera del intercambio.".into());
        }
        if std::fs::read("request.json").is_err() {
            return Err("El directorio de intercambio no está disponible.".into());
        }
        Ok(ExtensionOutput {
            kind: OutputKind::Text,
            text: "Intercambio disponible y archivo externo inaccesible".into(),
        })
    })
}
