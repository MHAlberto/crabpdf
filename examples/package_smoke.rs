//! Capture a real reader frame using the libraries and tools beside this executable.
use crabpdf::{
    config::{Preferences, ProfileStore},
    ui::ReaderApp,
};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Smoke {
    app: ReaderApp,
    output: PathBuf,
    started: Instant,
    ready: Option<Instant>,
    requested: bool,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.outcome.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
impl eframe::App for Smoke {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
            return;
        }
        let capture = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                _ => None,
            })
        });
        if let Some(image) = capture {
            let bytes: Vec<_> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            let result = image::save_buffer(self.output.join("reader.png"), &bytes,
                image.width() as u32, image.height() as u32, image::ColorType::Rgba8)
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    let details = self.app.state().details.as_ref().ok_or("Missing metadata")?;
                    let document = self.app.state().document.as_ref().ok_or("Missing document")?;
                    let report = serde_json::json!({"version": env!("CARGO_PKG_VERSION"),
                        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH,
                        "pages": document.info.page_count, "properties": details.properties,
                        "native_frame_captured": true, "seconds": self.started.elapsed().as_secs_f64()});
                    std::fs::write(self.output.join("validation.json"), serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
                });
            self.finish(ctx, result);
        } else if self.started.elapsed() > Duration::from_secs(45) {
            self.finish(ctx, Err("Package frame timed out".into()));
        }
        ctx.request_repaint_after(Duration::from_millis(25));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if self.app.has_current_render() && self.app.state().details.is_some() {
            let ready = self.ready.get_or_insert_with(Instant::now);
            if !self.requested && ready.elapsed() > Duration::from_millis(300) {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.requested = true;
            }
        }
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: package_smoke OUTPUT DOCUMENT".into());
    }
    let output = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let document = PathBuf::from(&args[1]).canonicalize()?;
    let config = output.join("preferences.toml");
    ProfileStore::save(
        &Preferences {
            restore_session: false,
            reduce_motion: true,
            ..Preferences::default()
        },
        &config,
    )?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF packaged reader smoke",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(cc, Some(document), Some(config)),
                output,
                started: Instant::now(),
                ready: None,
                requested: false,
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .map_err(|_| "Outcome mutex poisoned")?
        .take()
        .ok_or("Window closed before capture")?
        .map_err(Into::into)
}
