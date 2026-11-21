//! Optional local OCR adapter. The reader never loads Tesseract itself.
use crabpdf_extension_api::{ExtensionOutput, OutputKind, Request, serve};
use std::{
    fs::File,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("CRABPDF_TESSERACT").map(PathBuf::from) {
        return (path.is_absolute() && path.is_file()).then_some(path);
    }
    #[cfg(windows)]
    {
        ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|root| PathBuf::from(root).join("Tesseract-OCR/tesseract.exe"))
            .find(|path| path.is_file())
    }
    #[cfg(not(windows))]
    {
        [
            "/usr/bin/tesseract",
            "/usr/local/bin/tesseract",
            "/opt/homebrew/bin/tesseract",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
    }
}

fn recognize(request: Request) -> Result<ExtensionOutput, String> {
    let language = match request.command.as_str() {
        "recognize-spa" => "spa",
        "recognize-eng" => "eng",
        _ => return Err("Comando de OCR desconocido.".into()),
    };
    let image = request
        .image
        .ok_or("Selecciona una página o un área del PDF.")?;
    let program = executable().ok_or(
        "Instala Tesseract con el idioma elegido o indica su ruta absoluta en CRABPDF_TESSERACT.",
    )?;
    let scratch = std::env::var_os("CRABPDF_EXTENSION_SCRATCH")
        .map(PathBuf::from)
        .ok_or("Falta la carpeta temporal de CrabPDF.")?;
    let base = scratch.join("recognized");
    let mut command = Command::new(program);
    command
        .arg(&image.png_path)
        .arg(&base)
        .args(["-l", language])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("No se pudo iniciar Tesseract: {error}"))?;
    let began = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) => return Err(
                "Tesseract no terminó el reconocimiento. Comprueba que el idioma esté instalado."
                    .into(),
            ),
            Ok(None) if began.elapsed() < Duration::from_secs(90) => {
                std::thread::sleep(Duration::from_millis(30));
            }
            result => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(result.err().map_or_else(
                    || "Tesseract agotó el tiempo de reconocimiento.".into(),
                    |error| error.to_string(),
                ));
            }
        }
    }
    let mut text = String::new();
    File::open(base.with_extension("txt"))
        .map_err(|error| error.to_string())?
        .take(64 * 1024 + 1)
        .read_to_string(&mut text)
        .map_err(|error| error.to_string())?;
    if text.len() > 64 * 1024 {
        return Err("El texto reconocido supera el límite de una entrada.".into());
    }
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Err("No se reconoció texto en la selección.".into());
    }
    Ok(ExtensionOutput {
        kind: OutputKind::Text,
        text,
    })
}

fn main() {
    if let Err(error) = serve(recognize) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
