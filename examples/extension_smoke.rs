//! Native UI smoke for installed package discovery and capture from a PDF selection.
use crabpdf::{commands::Action, desktop::extensions::Repository, ui::ReaderApp};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

struct Smoke {
    app: ReaderApp,
    output: PathBuf,
    phase: u8,
    started: Instant,
    captured: bool,
    events: Vec<egui::Event>,
    outcome: Arc<Mutex<bool>>,
}
impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        input.focused = true;
        input
            .events
            .retain(|event| matches!(event, egui::Event::Screenshot { .. }));
        input.events.append(&mut self.events);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.started.elapsed() > Duration::from_secs(25) {
            eprintln!("Extension UI smoke timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        match self.phase {
            0 if self.app.has_current_render() && self.app.text_hit_position().is_some() => {
                self.app.execute(Action::ExtensionArea, ctx);
                let start = self.app.text_hit_position().unwrap() - egui::vec2(5.0, 5.0);
                self.events.push(egui::Event::PointerMoved(start));
                self.events.push(egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                });
                self.phase = 1;
                self.started = Instant::now();
            }
            1 if self.started.elapsed() > Duration::from_millis(150) => {
                let end = self.app.text_hit_position().unwrap() + egui::vec2(310.0, 120.0);
                self.events.push(egui::Event::PointerMoved(end));
                self.phase = 2;
                self.started = Instant::now();
            }
            2 if self.started.elapsed() > Duration::from_millis(120) => {
                let end = self.app.text_hit_position().unwrap() + egui::vec2(310.0, 120.0);
                self.events.push(egui::Event::PointerButton {
                    pos: end,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                });
                self.phase = 3;
                self.started = Instant::now();
            }
            3 if self.started.elapsed() > Duration::from_millis(800) && !self.captured => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.captured = true;
            }
            3 if self.started.elapsed() > Duration::from_millis(1800) => {
                // The fixture's selected package is shown in the desktop detail pane.
                let pos = egui::pos2(534.0, 541.0);
                self.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true, false] {
                    self.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                self.phase = 4;
                self.captured = false;
            }
            4 if self.app.extension_result().is_some_and(|result| {
                result.contains("Página 1") && result.contains("Recorte")
            }) =>
            {
                self.phase = 5;
                self.started = Instant::now();
            }
            5 if self.started.elapsed() > Duration::from_millis(400) && !self.captured => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.captured = true;
            }
            _ => {}
        }
        let screenshot = ctx.input(|input| {
            input.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                _ => None,
            })
        });
        if let Some(image) = screenshot {
            let screenshot_name = if self.phase < 4 {
                "manager.png"
            } else {
                "extensions.png"
            };
            let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            image::save_buffer(
                self.output.join(screenshot_name),
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            if self.phase >= 4 {
                *self.outcome.lock().unwrap() = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        ctx.request_repaint_after(Duration::from_millis(30));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("CRABPDF_DATA_DIR").is_some() {
        return Err("Unset CRABPDF_DATA_DIR to use this smoke test's isolated profile".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = root.join(format!("artifacts/extension-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&output)?;
    let data = output.join("data");
    let repository = Repository::new(&data);
    let source = root.join("artifacts/extensions/selection-info");
    if repository.list()?.is_empty() {
        repository.install(&source)?;
    }
    let package = repository.list()?.remove(0);
    repository.enable(&package, true)?;
    let pomodoro = root.join(format!(
        "artifacts/extension-build/pomodoro-{}",
        crabpdf::desktop::extensions::target()
    ));
    if pomodoro.is_dir() {
        repository.install(&pomodoro)?;
    }
    let outcome = Arc::new(Mutex::new(false));
    let result = Arc::clone(&outcome);
    // All profile and document data are scoped to this test's output directory.
    let source_pdf = output.join("sample.pdf");
    std::fs::copy(root.join("tests/fixtures/reading-sample.pdf"), &source_pdf)?;
    eframe::run_native(
        "CrabPDF extension integration",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(
                    cc,
                    Some(source_pdf),
                    Some(output.join("data/preferences.toml")),
                ),
                output,
                phase: 0,
                started: Instant::now(),
                captured: false,
                events: Vec::new(),
                outcome,
            }))
        }),
    )?;
    if !*result.lock().unwrap() {
        return Err("No UI screenshot captured".into());
    }
    Ok(())
}
