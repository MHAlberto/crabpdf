//! Native integration test of argument parsing and asynchronous startup options.
use crabpdf::cli::{self, Invocation};
use crabpdf::document::Zoom;
use crabpdf::layout::LayoutMode;
use crabpdf::ui::ReaderApp;
use eframe::egui;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct StartupSmoke {
    app: ReaderApp,
    started: Instant,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
impl eframe::App for StartupSmoke {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        let result = if self.app.has_current_render() {
            let state = self.app.state();
            if self.app.tab_count() == 2
                && state
                    .document
                    .as_ref()
                    .is_some_and(|d| d.page == 1 && d.info.file_name() == "features-sample.pdf")
                && state.zoom == Zoom::Percent(200)
                && state.layout == LayoutMode::Facing
                && state.rotation == 1
            {
                Some(Ok(()))
            } else {
                Some(Err(
                    "Las opciones CLI no se aplicaron a la pestaña activa.".into()
                ))
            }
        } else if self.started.elapsed() > Duration::from_secs(30) {
            Some(Err("Timeout: la página de destino debe quedar visible también con dos páginas y zoom alto.".into()))
        } else {
            self.app
                .state()
                .error
                .as_ref()
                .map(|error| Err(error.to_string()))
        };
        if let Some(result) = result {
            if let Ok(mut outcome) = self.outcome.lock() {
                *outcome = Some(result);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config = root.join(format!(
        "artifacts/cli-smoke-{}.toml",
        std::env::consts::ARCH
    ));
    let args: Vec<OsString> = vec![
        "--page".into(),
        "2".into(),
        "--zoom".into(),
        "200".into(),
        "--view".into(),
        "facing".into(),
        "--rotate".into(),
        "90".into(),
        "--config".into(),
        config.into_os_string(),
        root.join("tests/fixtures/reading-sample.pdf")
            .into_os_string(),
        root.join("tests/fixtures/features-sample.pdf")
            .into_os_string(),
    ];
    let Invocation::Run(launch) = cli::parse(args)? else {
        return Err("Se esperaba un arranque CLI".into());
    };
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF CLI test",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(StartupSmoke {
                app: ReaderApp::with_launch(cc, launch),
                started: Instant::now(),
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .map_err(|_| "Error de resultado")?
        .take()
        .ok_or("Prueba interrumpida")?
        .map_err(Into::into)
}
