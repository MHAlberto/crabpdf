//! Exercises the actual native eframe/Glow window with isolated preferences.
//! cargo run --example visual_smoke -- artifacts/ui
use crabpdf::commands::Action;
use crabpdf::document::{ErrorKind, Zoom};
use crabpdf::ui::ReaderApp;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct VisualSmoke {
    app: ReaderApp,
    output: PathBuf,
    phase: usize,
    capture_pending: bool,
    settled: Option<Instant>,
    started: Instant,
    pending_keys: Vec<egui::Key>,
    pending_text: Option<String>,
    pending_path: Option<PathBuf>,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}

const SHOTS: [&str; 25] = [
    "01-welcome",
    "02-page-one-fit",
    "03-page-two-fit",
    "04-page-two-zoom",
    "05-page-two-dark",
    "06-page-one-dark",
    "07-facing",
    "08-continuous-book",
    "09-tabs-outline",
    "10-search",
    "11-next-search-hit",
    "12-rotated-search",
    "13-inverted",
    "14-grayscale",
    "15-properties",
    "16-password",
    "17-unlocked",
    "18-missing-file",
    "19-invalid-file",
    "20-palette",
    "21-thumbnails",
    "22-selection",
    "23-duplicate-tab",
    "24-reopened-tab",
    "25-close-left",
];

impl VisualSmoke {
    fn finish(&mut self, ctx: &egui::Context, result: Result<(), String>) {
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = Some(result);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn advance(&mut self, ctx: &egui::Context) {
        self.phase += 1;
        self.capture_pending = false;
        self.settled = None;
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        match self.phase {
            1 => self.app.open_path(fixture.join("reading-sample.pdf")),
            2 => self.pending_keys.push(egui::Key::ArrowRight),
            3 => self.app.set_zoom(Zoom::Percent(200)),
            4 => {
                self.app.set_zoom(Zoom::FitWidth);
                self.app.toggle_theme(ctx);
            }
            5 => self.pending_keys.push(egui::Key::ArrowLeft),
            6 => {
                self.app.execute(Action::Facing, ctx);
                self.app.execute(Action::FitPage, ctx);
            }
            7 => {
                self.app.execute(Action::Book, ctx);
                if !self.app.state().continuous {
                    self.app.execute(Action::Continuous, ctx);
                }
                self.app.set_zoom(Zoom::Percent(50));
            }
            8 => {
                self.app.execute(Action::Outline, ctx);
                self.app.open_path(fixture.join("features-sample.pdf"));
            }
            9 => {
                self.app.execute(Action::Find, ctx);
                self.pending_text = Some("reader".into());
            }
            10 => self.app.execute(Action::FindNext, ctx),
            11 => {
                self.app.execute(Action::RotateRight, ctx);
                self.app.execute(Action::FitPage, ctx);
            }
            12 => self.app.execute(Action::Invert, ctx),
            13 => self.app.execute(Action::Grayscale, ctx),
            14 => self.app.execute(Action::Properties, ctx),
            15 => {
                self.pending_keys.push(egui::Key::Escape);
                self.pending_path = Some(fixture.join("protected-sample.pdf"));
            }
            16 => {
                self.pending_text = Some("reader-test".into());
                self.pending_keys.push(egui::Key::Enter);
            }
            17 => self.app.open_path(fixture.join("does-not-exist.pdf")),
            18 => self
                .app
                .open_path(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")),
            19 => {
                self.app.execute(Action::CloseAll, ctx);
                self.app.execute(Action::Palette, ctx);
            }
            20 => {
                self.pending_keys.push(egui::Key::Escape);
                self.app.execute(Action::Thumbnails, ctx);
                self.pending_path = Some(fixture.join("features-sample.pdf"));
            }
            21 => self.app.execute(Action::SelectAll, ctx),
            22 => self.app.execute(Action::DuplicateTab, ctx),
            23 => {
                self.app.execute(Action::Close, ctx);
                self.app.execute(Action::Reopen, ctx);
            }
            24 => self.app.execute(Action::CloseLeft, ctx),
            _ => self.finish(ctx, Ok(())),
        }
    }
    fn ready(&self) -> bool {
        match self.phase {
            0 | 19 => self.app.state().error.is_none(),
            1..=14 | 16 | 20..=24 => {
                let rendered = self.app.has_current_render() && self.app.state().document.is_some();
                let (hits, scanned, total) = self.app.search_progress();
                let search = !(9..=14).contains(&self.phase) || hits >= 5 && scanned == total;
                let page = self
                    .app
                    .state()
                    .document
                    .as_ref()
                    .map_or(usize::MAX, |d| d.page);
                rendered
                    && search
                    && match self.phase {
                        1 | 5 => page == 0,
                        2..=4 => page == 1,
                        8 => self.app.tab_count() == 2,
                        22 | 23 => self.app.tab_count() == 2,
                        24 => self.app.tab_count() == 1,
                        _ => true,
                    }
            }
            15 => self
                .app
                .state()
                .error
                .as_ref()
                .is_some_and(|e| e.kind == ErrorKind::Protected),
            17 => self
                .app
                .state()
                .error
                .as_ref()
                .is_some_and(|e| e.kind == ErrorKind::File),
            18 => self
                .app
                .state()
                .error
                .as_ref()
                .is_some_and(|e| e.kind == ErrorKind::InvalidPdf),
            _ => false,
        }
    }
}
impl eframe::App for VisualSmoke {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Some(text) = self.pending_text.take() {
            input.events.push(egui::Event::Text(text));
        }
        for key in self.pending_keys.drain(..) {
            for pressed in [true, false] {
                input.events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if let Some(path) = self.pending_path.take() {
            self.app.open_path(path);
        }
        if self.started.elapsed() > Duration::from_secs(90) {
            self.finish(
                ctx,
                Err(format!(
                    "Visual smoke timeout in phase {} (search {:?})",
                    self.phase,
                    self.app.search_progress()
                )),
            );
            return;
        }
        let screenshots = ctx.input(|input| {
            input
                .events
                .iter()
                .filter_map(|event| {
                    if let egui::Event::Screenshot { image, .. } = event {
                        Some(Arc::clone(image))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        for screenshot in screenshots {
            if !self.capture_pending || self.phase >= SHOTS.len() {
                continue;
            }
            let path = self.output.join(format!("{}.png", SHOTS[self.phase]));
            let bytes: Vec<u8> = screenshot
                .pixels
                .iter()
                .flat_map(|p| p.to_array())
                .collect();
            if let Err(error) = image::save_buffer(
                &path,
                &bytes,
                screenshot.width() as u32,
                screenshot.height() as u32,
                image::ColorType::Rgba8,
            ) {
                self.finish(ctx, Err(error.to_string()));
                return;
            }
            println!("Saved {}", path.display());
            self.advance(ctx);
        }
        if self.ready() && !self.capture_pending {
            let settled = self.settled.get_or_insert_with(Instant::now);
            if settled.elapsed() > Duration::from_millis(250) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture_pending = true;
            }
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/ui"));
    std::fs::create_dir_all(&output)?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    let options = eframe::NativeOptions {
        viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "crabpdf-visual-smoke",
        options,
        Box::new(move |cc| {
            Ok(Box::new(VisualSmoke {
                // None gives default preferences and performs no writes outside the workspace.
                app: ReaderApp::with_config(cc, None, None),
                output,
                phase: 0,
                capture_pending: false,
                settled: None,
                started: Instant::now(),
                pending_keys: Vec::new(),
                pending_text: None,
                pending_path: None,
                outcome,
            }))
        }),
    )?;
    let status = result
        .lock()
        .map_err(|_| "Cannot read smoke test result")?
        .take()
        .ok_or("Window closed before smoke test finished")?;
    status.map_err(Into::into)
}
