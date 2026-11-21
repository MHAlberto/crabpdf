//! Print setup and asynchronous spooler submission.
use super::*;
use crate::desktop::printing::{self, Scale, TemporaryPdf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};

struct PrintJob {
    document: DocumentId,
    file: TemporaryPdf,
    cancelled: Arc<AtomicBool>,
    printer: String,
    scale: Scale,
}

enum Job {
    Preparing(PrintJob),
    Submitting {
        cancelled: Arc<AtomicBool>,
        receive: mpsc::Receiver<std::io::Result<bool>>,
    },
}

#[derive(Default)]
pub(super) struct PrintDialog {
    pub(super) open: bool,
    range: String,
    fit: bool,
    printer: String,
    available: Vec<String>,
    discovery: Option<mpsc::Receiver<std::io::Result<Vec<String>>>>,
    job: Option<Job>,
    message: Option<String>,
}

impl PrintDialog {
    fn discover(&mut self, ctx: &egui::Context) {
        if self.discovery.is_some() {
            return;
        }
        let (send, receive) = mpsc::channel();
        let repaint = ctx.clone();
        match std::thread::Builder::new()
            .name("crabpdf-printers".into())
            .spawn(move || {
                let _ = send.send(printing::printers());
                repaint.request_repaint();
            }) {
            Ok(_) => self.discovery = Some(receive),
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    pub(super) fn prepared(
        &mut self,
        ctx: &egui::Context,
        document: DocumentId,
        result: Result<(), ReaderError>,
    ) {
        if !matches!(&self.job, Some(Job::Preparing(job)) if job.document == document) {
            return;
        }
        let Some(Job::Preparing(job)) = self.job.take() else {
            return;
        };
        if job.cancelled.load(Ordering::Relaxed) {
            self.message = Some("Impresión cancelada.".into());
            return;
        }
        if let Err(error) = result {
            self.message = Some(error.to_string());
            return;
        }
        let (send, receive) = mpsc::channel();
        let cancelled = Arc::clone(&job.cancelled);
        let repaint = ctx.clone();
        match std::thread::Builder::new()
            .name("crabpdf-print-submit".into())
            .spawn(move || {
                let result =
                    printing::submit(&job.file.path, &job.printer, job.scale, &job.cancelled);
                let _ = send.send(result);
                repaint.request_repaint();
            }) {
            Ok(_) => self.job = Some(Job::Submitting { cancelled, receive }),
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn poll(&mut self) {
        if let Some(receive) = &self.discovery {
            match receive.try_recv() {
                Ok(Ok(printers)) => {
                    self.available = printers;
                    self.discovery = None;
                }
                Ok(Err(error)) => {
                    self.message = Some(error.to_string());
                    self.discovery = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.discovery = None;
                    self.message = Some("Se interrumpió la búsqueda de impresoras.".into());
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(Job::Submitting { receive, .. }) = &self.job {
            let status = match receive.try_recv() {
                Ok(Ok(true)) => "Trabajo enviado a la cola del sistema.".to_owned(),
                Ok(Ok(false)) => "Impresión cancelada.".to_owned(),
                Ok(Err(error)) => error.to_string(),
                Err(mpsc::TryRecvError::Disconnected) => {
                    "Se interrumpió el envío a la impresora.".into()
                }
                Err(mpsc::TryRecvError::Empty) => return,
            };
            self.job = None;
            self.message = Some(status);
        }
    }
}

impl ReaderApp {
    pub(super) fn open_print(&mut self, ctx: &egui::Context) {
        self.printing.open = true;
        if self.printing.job.is_none() {
            self.printing.discover(ctx);
        }
        ctx.request_repaint();
    }

    pub(super) fn show_printing(&mut self, ctx: &egui::Context) {
        let active = self
            .state()
            .document
            .as_ref()
            .map(|doc| (doc.id, doc.page, doc.info.page_count));
        let dialog = &mut self.printing;
        dialog.poll();
        if matches!(&dialog.job, Some(Job::Preparing(job)) if self.worker.is_none() || !self.tabs.iter().any(|tab| tab.reader.id == job.document))
        {
            dialog.job = None;
            dialog.message = Some("El documento se cerró antes de preparar el trabajo.".into());
        }
        if !dialog.open {
            return;
        }
        let mut open = true;
        let mut start = None;
        egui::Window::new("Imprimir PDF")
            .open(&mut open)
            .default_width(390.0)
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(message) = &dialog.message {
                    ui.label(message);
                }
                if let Some(job) = &dialog.job {
                    let cancel = match job {
                        Job::Preparing(job) => {
                            ui.label("Preparando las páginas…");
                            &job.cancelled
                        }
                        Job::Submitting { cancelled, .. } => {
                            ui.label("Enviando a la impresora…");
                            cancelled
                        }
                    };
                    if ui
                        .add_enabled(
                            !cancel.load(Ordering::Relaxed),
                            egui::Button::new("Cancelar trabajo"),
                        )
                        .clicked()
                    {
                        cancel.store(true, Ordering::Relaxed);
                    }
                    return;
                }
                let Some((document, page, total)) = active else {
                    ui.label("Abre un documento para imprimir.");
                    return;
                };
                egui::ComboBox::from_label("Impresora")
                    .selected_text(if dialog.printer.is_empty() {
                        "Predeterminada"
                    } else {
                        &dialog.printer
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut dialog.printer, String::new(), "Predeterminada");
                        for printer in &dialog.available {
                            ui.selectable_value(&mut dialog.printer, printer.clone(), printer);
                        }
                    });
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{total} páginas"));
                    if ui.button("Todas").clicked() {
                        dialog.range.clear();
                    }
                    if ui.button("Página actual").clicked() {
                        dialog.range = (page + 1).to_string();
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut dialog.range)
                        .hint_text("Rango opcional, por ejemplo 2,4-7")
                        .char_limit(256),
                );
                ui.checkbox(&mut dialog.fit, "Ajustar al área imprimible");
                let range = crabpdf_application::printing::pages(&dialog.range, total);
                if let Err(error) = &range {
                    ui.label(*error);
                }
                if ui
                    .add_enabled(range.is_ok(), egui::Button::new("Imprimir"))
                    .clicked()
                    && let Ok(range) = range
                {
                    start = Some((document, range));
                }
            });
        dialog.open = open;
        if let Some((document, range)) = start {
            let job = TemporaryPdf::create(&self.app_paths.data)
                .map_err(|e| e.to_string())
                .and_then(|file| {
                    self.worker
                        .as_ref()
                        .ok_or_else(|| "El lector no está disponible.".to_owned())?
                        .prepare_print(document, file.path.clone(), range)
                        .map_err(|e| e.to_string())?;
                    Ok(PrintJob {
                        document,
                        file,
                        cancelled: Arc::new(AtomicBool::new(false)),
                        printer: dialog.printer.clone(),
                        scale: if dialog.fit {
                            Scale::Fit
                        } else {
                            Scale::Actual
                        },
                    })
                });
            match job {
                Ok(job) => {
                    dialog.message = None;
                    dialog.job = Some(Job::Preparing(job));
                }
                Err(error) => dialog.message = Some(error),
            }
        }
    }
}
