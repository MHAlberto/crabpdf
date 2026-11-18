//! Reference extension. Reports its input to exercise the package API.
use crabpdf_extension_api::{ExtensionOutput, OutputKind, serve};
fn main() -> std::io::Result<()> {
    serve(|request| {
        if request.command != "inspect" {
            return Err("Unknown command".into());
        }
        let image = request.image.ok_or("This command expects an image")?;
        let bytes = std::fs::metadata(&image.png_path)
            .map_err(|e| e.to_string())?
            .len();
        Ok(ExtensionOutput {
            kind: OutputKind::Text,
            text: format!(
                "Página {}
Recorte {} × {} px
PNG de {} bytes
Texto del área
{}",
                request.page,
                image.width,
                image.height,
                bytes,
                request.text.unwrap_or_default()
            ),
        })
    })
}
