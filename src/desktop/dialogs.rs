/// Construct the async dialog on the UI thread (required for reliable macOS use).
pub fn pdf_dialog() -> impl std::future::Future<Output = Option<rfd::FileHandle>> + Send {
    rfd::AsyncFileDialog::new()
        .set_title("Abrir un documento")
        .add_filter(
            "Documentos compatibles",
            crabpdf_application::formats::EXTENSIONS,
        )
        .pick_file()
}

use crabpdf_core::document::DocumentId;
use eframe::egui;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};

struct RepaintWake(egui::Context);
impl Wake for RepaintWake {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}
type DialogFuture = Pin<Box<dyn Future<Output = Option<Vec<rfd::FileHandle>>> + Send>>;
pub struct PendingDialog {
    document: Option<DocumentId>,
    future: DialogFuture,
}
impl PendingDialog {
    /// Construct on the UI thread, including on macOS.
    pub fn open() -> Self {
        let future = rfd::AsyncFileDialog::new()
            .set_title("Abrir documentos")
            .add_filter(
                "Documentos compatibles",
                crabpdf_application::formats::EXTENSIONS,
            )
            .add_filter("PDF", &["pdf", "ai"])
            .add_filter(
                "Libros",
                &[
                    "epub", "mobi", "azw", "azw3", "azw4", "prc", "pdb", "fb2", "fb2z", "fbz",
                    "zfb2",
                ],
            )
            .add_filter(
                "Cómics",
                &["cbz", "cbr", "cb7", "cbt", "zip", "rar", "7z", "tar"],
            )
            .add_filter(
                "DjVu / XPS / CHM",
                &["djvu", "djv", "xps", "oxps", "xod", "dwfx", "chm"],
            )
            .add_filter("Todos los archivos", &["*"])
            .pick_files();
        Self {
            document: None,
            future: Box::pin(future),
        }
    }
    pub fn save(id: DocumentId, name: String) -> Self {
        let future = rfd::AsyncFileDialog::new()
            .set_title("Guardar copia PDF con las anotaciones")
            .add_filter("PDF", &["pdf"])
            .set_file_name(name)
            .save_file();
        Self {
            document: Some(id),
            future: Box::pin(async move { future.await.map(|file| vec![file]) }),
        }
    }
    /// Some with an empty vector also signals dialog cancellation/completion.
    pub fn poll(&mut self, ctx: &egui::Context) -> Option<(Option<DocumentId>, Vec<PathBuf>)> {
        let waker = Waker::from(Arc::new(RepaintWake(ctx.clone())));
        let mut task = Context::from_waker(&waker);
        match self.future.as_mut().poll(&mut task) {
            Poll::Pending => None,
            Poll::Ready(files) => Some((
                self.document,
                files
                    .unwrap_or_default()
                    .into_iter()
                    .map(|file| file.path().to_owned())
                    .collect(),
            )),
        }
    }
}
