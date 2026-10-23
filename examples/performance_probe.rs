//! Reproducible UI readiness timings. No screenshot or compositor latency claims.
use crabpdf::{commands::Action, config::Preferences, document::Zoom, ui::ReaderApp};
use eframe::egui;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Serialize)]
struct Sample {
    phase: &'static str,
    elapsed_ms: f64,
}
#[derive(Serialize)]
struct Report {
    schema: u32,
    version: &'static str,
    architecture: &'static str,
    os: &'static str,
    viewport_points: [f32; 2],
    pixels_per_point: f32,
    samples: Vec<Sample>,
    idle_logic_calls: u64,
    idle_seconds: f64,
    notes: &'static str,
}
struct Probe {
    app: ReaderApp,
    pdf: PathBuf,
    epub: PathBuf,
    output: PathBuf,
    phase: u8,
    began: Instant,
    startup: Instant,
    samples: Vec<Sample>,
    scroll_y: f32,
    scroll_changed: Instant,
    idle_calls: u64,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}
impl Probe {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.result.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn phase(&mut self, phase: u8) {
        self.phase = phase;
        // Consumed by the external observer, outside the timed operation.
        std::fs::write(self.output.join("phase.txt"), phase.to_string()).unwrap();
        self.began = Instant::now();
    }
    fn record(&mut self, name: &'static str) {
        self.samples.push(Sample {
            phase: name,
            elapsed_ms: self.began.elapsed().as_secs_f64() * 1000.0,
        });
    }
    fn ready(&self) -> bool {
        self.app.has_current_render() && self.app.current_page_viewport().is_some()
    }
    fn step(&mut self, ctx: &egui::Context) {
        let page = self.app.state().document.as_ref().map(|d| d.page);
        match self.phase {
            0 if self.ready() => {
                self.record("pdf_first_open");
                self.samples.push(Sample {
                    phase: "startup_to_first_ready",
                    elapsed_ms: self.startup.elapsed().as_secs_f64() * 1000.0,
                });
                if self.app.state().continuous {
                    self.app.execute(Action::Continuous, ctx);
                }
                self.phase(1);
            }
            1 if self.ready() && self.app.prefetch_ready(1) => {
                self.phase(2);
                self.app.navigate(true);
            }
            2 if self.ready() && page == Some(1) => {
                self.record("page_next_prefetched");
                let last = self.app.state().document.as_ref().unwrap().info.page_count - 1;
                if last <= 3 || self.app.prefetch_ready(last) {
                    self.finish(
                        ctx,
                        Err("Fixture must have a non-prefetched last page".into()),
                    );
                    return;
                }
                self.phase(3);
                self.app.execute(Action::Last, ctx);
            }
            3 if self.ready()
                && page
                    == self
                        .app
                        .state()
                        .document
                        .as_ref()
                        .map(|d| d.info.page_count - 1) =>
            {
                self.record("page_last_not_prefetched");
                self.app.execute(Action::First, ctx);
                self.app.execute(Action::Continuous, ctx);
                self.app.set_zoom(Zoom::Percent(150));
                self.phase(4);
            }
            4 if self.ready() => {
                self.scroll_y = self.app.scroll_offset().unwrap().y;
                self.scroll_changed = Instant::now();
                self.phase(5);
                self.app.execute(Action::ScrollDownPage, ctx);
            }
            5 => {
                let y = self.app.scroll_offset().unwrap().y;
                if (y - self.scroll_y).abs() > 0.5 {
                    self.scroll_y = y;
                    self.scroll_changed = Instant::now();
                }
                if y > 100.0
                    && self.ready()
                    && self.scroll_changed.elapsed() > Duration::from_millis(80)
                {
                    self.record("scroll_ready_and_stable_80ms");
                    self.app.execute(Action::Close, ctx);
                    self.phase(6);
                    self.app.open_path(self.pdf.clone());
                }
            }
            6 if self.ready() => {
                self.record("pdf_reopen_same_process");
                self.app.execute(Action::Close, ctx);
                self.phase(7);
                self.app.open_path(self.epub.clone());
            }
            7 if self.ready() => {
                self.record("epub_first_open_empty_import_cache");
                self.phase(8);
            }
            8 if self.began.elapsed() > Duration::from_secs(1) => {
                self.phase(9);
                self.idle_calls = 0;
            }
            9 if self.began.elapsed() >= Duration::from_secs(4) => {
                let report = Report {
                    schema: 1,
                    version: env!("CARGO_PKG_VERSION"),
                    architecture: std::env::consts::ARCH,
                    os: std::env::consts::OS,
                    viewport_points: [ctx.viewport_rect().width(), ctx.viewport_rect().height()],
                    pixels_per_point: ctx.pixels_per_point(),
                    samples: std::mem::take(&mut self.samples),
                    idle_logic_calls: self.idle_calls,
                    idle_seconds: self.began.elapsed().as_secs_f64(),
                    notes: "Readiness is sampled after UI submission; not physical presentation. OS disk cache is uncontrolled. Scroll includes motion and an 80ms stability gate. Idle uses a single 4s observation timer.",
                };
                let result = serde_json::to_vec_pretty(&report)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        std::fs::write(self.output.join("timings.json"), bytes)
                            .map_err(|e| e.to_string())
                    });
                self.phase(10);
                self.finish(ctx, result);
            }
            _ => {}
        }
        if self.phase == 9 {
            ctx.request_repaint_after(Duration::from_secs(4).saturating_sub(self.began.elapsed()));
        } else {
            ctx.request_repaint_after(Duration::from_millis(8));
        }
    }
}
impl eframe::App for Probe {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.phase == 9 {
            self.idle_calls += 1;
        }
        self.app.logic(ctx, frame);
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
        }
        if self.began.elapsed() > Duration::from_secs(60) {
            self.finish(ctx, Err(format!("Timeout in phase {}", self.phase)));
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("Usage: performance_probe OUTPUT PDF EPUB".into());
    }
    let output = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let pdf = PathBuf::from(&args[1]).canonicalize()?;
    let epub = PathBuf::from(&args[2]).canonicalize()?;
    let config = output.join("preferences.toml");
    Preferences::default().save(&config)?;
    let result = Arc::new(Mutex::new(None));
    let outcome = Arc::clone(&result);
    let startup = Instant::now();
    eframe::run_native(
        "CrabPDF architecture performance probe",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            let began = Instant::now();
            Ok(Box::new(Probe {
                app: ReaderApp::with_config(cc, Some(pdf.clone()), Some(config)),
                pdf,
                epub,
                output,
                phase: 0,
                began,
                startup,
                samples: Vec::new(),
                scroll_y: 0.0,
                scroll_changed: Instant::now(),
                idle_calls: 0,
                result,
            }))
        }),
    )?;
    outcome
        .lock()
        .map_err(|_| "Outcome mutex poisoned")?
        .take()
        .ok_or("Probe interrupted")?
        .map_err(Into::into)
}
