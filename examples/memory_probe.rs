//! Repeated multi-book navigation for an external process-memory observer.
use crabpdf::{commands::Action, config::Preferences, document::Zoom, ui::ReaderApp};
use eframe::egui;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Serialize)]
struct Visit {
    round: usize,
    book: usize,
    page: usize,
    ready_ms: f64,
}
struct Probe {
    app: ReaderApp,
    books: Vec<PathBuf>,
    output: PathBuf,
    round: usize,
    book: usize,
    page: usize,
    started: Instant,
    settled: Option<Instant>,
    closing: bool,
    visits: Vec<Visit>,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}
impl Probe {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.result.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn mark_phase(&mut self) {
        let phase = if self.closing {
            "closed".into()
        } else {
            format!("round-{}-book-{}", self.round + 1, self.book + 1)
        };
        std::fs::write(self.output.join("phase.txt"), phase).unwrap();
        self.started = Instant::now();
        self.settled = None;
    }
    fn step(&mut self, ctx: &egui::Context) {
        if self.started.elapsed() > Duration::from_secs(30) {
            self.finish(ctx, Err("Memory probe timed out".into()));
            return;
        }
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
            return;
        }
        if !self.closing && self.app.state().document.is_some() && self.app.state().continuous {
            self.app.execute(Action::Continuous, ctx);
            self.settled = None;
            ctx.request_repaint();
            return;
        }
        if self.closing {
            if self.started.elapsed() >= Duration::from_secs(2) {
                let report = serde_json::json!({
                    "schema": 1, "version": env!("CARGO_PKG_VERSION"),
                    "architecture": std::env::consts::ARCH, "visits": self.visits,
                    "notes": "Three copies of the supplied fixture, two full sweeps, 125% zoom, 1080x780 viewport. Readiness is UI availability, not physical presentation. Memory is sampled by Test-Memory.ps1; excludes children and GPU."
                });
                let result = serde_json::to_vec_pretty(&report)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        std::fs::write(self.output.join("timings.json"), bytes)
                            .map_err(|e| e.to_string())
                    });
                self.finish(ctx, result);
            }
        } else if self.app.has_current_render()
            && self.app.current_page_viewport().is_some()
            && self
                .app
                .state()
                .document
                .as_ref()
                .is_some_and(|d| d.page == self.page)
        {
            let settled = self.settled.get_or_insert_with(Instant::now);
            if settled.elapsed() >= Duration::from_millis(100) {
                self.visits.push(Visit {
                    round: self.round + 1,
                    book: self.book + 1,
                    page: self.page + 1,
                    ready_ms: self.started.elapsed().as_secs_f64() * 1000.0,
                });
                let total = self
                    .app
                    .state()
                    .document
                    .as_ref()
                    .unwrap()
                    .info
                    .page_count
                    .min(24);
                if self.page + 1 < total {
                    self.page += 1;
                    self.app.navigate(true);
                } else {
                    self.book += 1;
                    self.page = 0;
                    if self.book == self.books.len() {
                        self.book = 0;
                        self.round += 1;
                    }
                    if self.round == 2 {
                        self.app.execute(Action::CloseAll, ctx);
                        self.closing = true;
                    } else {
                        self.app.open_path(self.books[self.book].clone());
                        self.app.execute(Action::First, ctx);
                        if self.app.state().continuous {
                            self.app.execute(Action::Continuous, ctx);
                        }
                        self.app.set_zoom(Zoom::Percent(125));
                    }
                }
                self.mark_phase();
            }
        }
        ctx.request_repaint_after(Duration::from_millis(8));
    }
}
impl eframe::App for Probe {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        // Timers can invoke logic without painting.
        if self.closing {
            self.step(ctx);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        self.step(ui.ctx());
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}

// Optional native screenshot, kept outside the measured navigation path.
struct PropertiesCapture {
    app: ReaderApp,
    output: PathBuf,
    opened: bool,
    requested: bool,
    started: Instant,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}
impl eframe::App for PropertiesCapture {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        let capture = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                _ => None,
            })
        });
        let result = if let Some(image) = capture {
            let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            Some(
                image::save_buffer(
                    self.output.join("properties.png"),
                    &bytes,
                    image.width() as u32,
                    image.height() as u32,
                    image::ColorType::Rgba8,
                )
                .map_err(|e| e.to_string()),
            )
        } else if let Some(error) = &self.app.state().error {
            Some(Err(error.to_string()))
        } else if self.started.elapsed() > Duration::from_secs(30) {
            Some(Err("Properties capture timed out".into()))
        } else {
            None
        };
        if let Some(result) = result {
            *self.result.lock().unwrap() = Some(result);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if !self.opened && self.app.has_current_render() && self.app.state().details.is_some() {
            self.app.execute(Action::Properties, ui.ctx());
            self.opened = true;
            self.started = Instant::now();
        } else if self.opened
            && !self.requested
            && self.started.elapsed() > Duration::from_millis(400)
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.requested = true;
        }
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let properties = args.len() == 3 && args[2] == "--properties";
    if args.len() != 2 && !properties {
        return Err("Usage: memory_probe OUTPUT PDF [--properties]".into());
    }
    let output = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let mut books = Vec::new();
    for index in 1..=3 {
        let path = output.join(format!("book-{index}.pdf"));
        std::fs::copy(&args[1], &path)?;
        books.push(path);
    }
    let config = output.join("preferences.toml");
    let mut preferences = Preferences::default();
    if properties {
        preferences.theme = crabpdf::config::Theme::Dark;
    }
    crabpdf::config::ProfileStore::save(&preferences, &config)?;
    let result = Arc::new(Mutex::new(None));
    let outcome = Arc::clone(&result);
    eframe::run_native(
        "CrabPDF memory probe",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            let mut app = ReaderApp::with_config(cc, Some(books[0].clone()), Some(config));
            if properties {
                return Ok(Box::new(PropertiesCapture {
                    app,
                    output,
                    opened: false,
                    requested: false,
                    started: Instant::now(),
                    result,
                }) as Box<dyn eframe::App>);
            }
            app.set_zoom(Zoom::Percent(125));
            let mut probe = Probe {
                app,
                books,
                output,
                round: 0,
                book: 0,
                page: 0,
                started: Instant::now(),
                settled: None,
                closing: false,
                visits: Vec::new(),
                result,
            };
            probe.mark_phase();
            Ok(Box::new(probe))
        }),
    )?;
    outcome
        .lock()
        .map_err(|_| "Outcome mutex poisoned")?
        .take()
        .ok_or("Probe interrupted")?
        .map_err(Into::into)
}
