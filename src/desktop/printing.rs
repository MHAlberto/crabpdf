//! Sends a prepared PDF through the OS printing service.
use std::{
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scale {
    Actual,
    Fit,
}

pub struct TemporaryPdf {
    pub path: PathBuf,
}

impl TemporaryPdf {
    pub fn create(root: &Path) -> io::Result<Self> {
        let parent = root.join("print");
        std::fs::create_dir_all(&parent)?;
        let id = getrandom::u64().map_err(io::Error::other)?;
        let folder = parent.join(format!("{id:016x}"));
        std::fs::create_dir(&folder)?;
        Ok(Self {
            path: folder.join("CrabPDF.pdf"),
        })
    }
}

impl Drop for TemporaryPdf {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

#[cfg(windows)]
pub fn printers() -> io::Result<Vec<String>> {
    Ok(winprint::printer::PrinterDevice::all()
        .map_err(io::Error::other)?
        .into_iter()
        .map(|printer| printer.os_name().to_string_lossy().into_owned())
        .collect())
}

#[cfg(unix)]
pub fn printers() -> io::Result<Vec<String>> {
    let output = std::process::Command::new("/usr/bin/lpstat")
        .arg("-e")
        .output()
        .map_err(|error| io::Error::other(format!("No se pudo consultar CUPS: {error}")))?;
    if !output.status.success() {
        return Err(io::Error::other("CUPS no pudo consultar sus colas."));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty() && line.len() < 256)
        .take(256)
        .map(str::to_owned)
        .collect())
}

#[cfg(windows)]
pub fn submit(path: &Path, device: &str, scale: Scale, cancelled: &AtomicBool) -> io::Result<bool> {
    use std::{ops::ControlFlow, time::Duration};
    use winprint::{
        printer::{FilePrinter, PrintOutcome, PrinterDevice, TimedPrinter, WinPdfPrinter},
        ticket::{PrintTicket, PrintTicketBuilder},
    };
    if cancelled.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let printer = if device.is_empty() {
        PrinterDevice::user_default().map_err(io::Error::other)?
    } else {
        PrinterDevice::all()
            .map_err(io::Error::other)?
            .into_iter()
            .find(|p| p.os_name() == device)
    }
    .ok_or_else(|| io::Error::other("La impresora seleccionada no está disponible."))?;
    let option = match scale {
        Scale::Actual => "None",
        Scale::Fit => "FitApplicationMediaSizeToPageImageableSize",
    };
    let delta = format!(
        r#"<psf:PrintTicket xmlns:psf="http://schemas.microsoft.com/windows/2003/08/printing/printschemaframework" xmlns:psk="http://schemas.microsoft.com/windows/2003/08/printing/printschemakeywords" version="1"><psf:Feature name="psk:PageScaling"><psf:Option name="psk:{option}"/></psf:Feature></psf:PrintTicket>"#
    );
    let mut ticket = PrintTicketBuilder::new(&printer).map_err(io::Error::other)?;
    ticket
        .merge(PrintTicket::from_xml(delta.into_bytes()))
        .map_err(io::Error::other)?;
    let mut driver = WinPdfPrinter::new(printer);
    driver.override_render_dpi(300, 300);
    driver.set_timeout(Some(Duration::from_secs(180)));
    let result = driver
        .print_with_progress(path, ticket.build().map_err(io::Error::other)?, &mut |_| {
            if cancelled.load(Ordering::Relaxed) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .map_err(io::Error::other)?;
    Ok(result == PrintOutcome::Completed)
}

#[cfg(unix)]
pub fn submit(path: &Path, device: &str, scale: Scale, cancelled: &AtomicBool) -> io::Result<bool> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if cancelled.load(Ordering::Relaxed) {
        return Ok(false);
    }
    let mut command = Command::new("/usr/bin/lp");
    if !device.is_empty() {
        command.args(["-d", device]);
    }
    let scale = match scale {
        Scale::Actual => "print-scaling=none",
        Scale::Fit => "print-scaling=fit",
    };
    let mut child = command
        .args(["-o", scale, "--"])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let began = Instant::now();
    loop {
        let state = child.try_wait();
        if let Ok(Some(status)) = state {
            return status
                .success()
                .then_some(true)
                .ok_or_else(|| io::Error::other("CUPS rechazó el trabajo de impresión."));
        }
        let interrupted = cancelled.load(Ordering::Relaxed);
        if interrupted || state.is_err() || began.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            let _ = child.wait();
            return if interrupted {
                Ok(false)
            } else {
                Err(state.err().unwrap_or_else(|| {
                    io::Error::other("CUPS tardó demasiado en aceptar el trabajo.")
                }))
            };
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
