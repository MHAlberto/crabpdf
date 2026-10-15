//! Regression: a result near the bottom of a previously unknown tall page must be visible.
use crabpdf::cli::{self, Invocation};
use crabpdf::commands::Action;
use crabpdf::ui::ReaderApp;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct SearchSmoke {
    app: ReaderApp,
    advanced: bool,
    started: Instant,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
impl eframe::App for SearchSmoke {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        let (hits, scanned, total) = self.app.search_progress();
        if !self.advanced && hits >= 6 && scanned == total {
            for _ in 1..hits {
                self.app.execute(Action::FindNext, ctx);
            }
            self.advanced = true;
        }
        let result = if self.started.elapsed() > Duration::from_secs(30) {
            Some(Err("Timeout al revelar el resultado de búsqueda.".into()))
        } else if self.advanced && self.app.has_current_render() {
            if let Some(viewport) = self.app.current_page_viewport() {
                let visible = self.app.current_search_hit().is_some_and(|hit| {
                    hit.page == 1 && hit.bounds.iter().any(|bounds| bounds.intersects(&viewport))
                });
                Some(if visible {
                    Ok(())
                } else {
                    Err("El resultado de la página alta quedó fuera del viewport.".into())
                })
            } else {
                None
            }
        } else {
            self.app.state().error.as_ref().map(|e| Err(e.to_string()))
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
    let args = vec![
        "--search".into(),
        "reader".into(),
        "--zoom".into(),
        "fit-width".into(),
        "--view".into(),
        "single".into(),
        "--rotate".into(),
        "0".into(),
        "--config".into(),
        root.join(format!(
            "artifacts/search-smoke-{}.toml",
            std::env::consts::ARCH
        ))
        .into_os_string(),
        root.join("examples/tall-sample.pdf").into_os_string(),
    ];
    let Invocation::Run(launch) = cli::parse(args)? else {
        return Err("Arranque inválido".into());
    };
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF search regression",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(SearchSmoke {
                app: ReaderApp::with_launch(cc, launch),
                advanced: false,
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
