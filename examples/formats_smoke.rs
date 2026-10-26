//! Native UI regression: word selection, colored marks, persistence and non-PDF documents.
use crabpdf::{commands::Action, ui::ReaderApp};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
const SHOTS: &[&str] = &[
    "01-pdf",
    "02-word-selection",
    "03-yellow-highlight",
    "04-green-highlight",
    "05-blue-underline",
    "06-undo",
    "07-reopened-marks",
    "08-epub",
    "09-epub-pink-mark",
    "10-cbr",
    "11-comic-next-page",
    "12-xps",
    "13-djvu-text",
    "14-chm",
    "15-library",
    "16-drm-message",
];
struct Smoke {
    app: ReaderApp,
    output: PathBuf,
    ebook: PathBuf,
    phase: usize,
    capture: bool,
    settled: Option<Instant>,
    started: Instant,
    keys: Vec<(egui::Modifiers, egui::Key)>,
    pointer: Option<egui::Pos2>,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("artifacts/format-samples")
        .join(name)
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.outcome.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn advance(&mut self, ctx: &egui::Context) {
        self.phase += 1;
        self.capture = false;
        self.settled = None;
        self.started = Instant::now();
        match self.phase {
            1 => self.pointer = self.app.text_hit_position(),
            2 => self.keys.push((
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::L,
            )),
            3 => {
                self.app.set_markup_color([90, 210, 100]);
                self.app.execute(Action::SelectAll, ctx);
                self.app.execute(Action::Highlight, ctx);
            }
            4 => {
                self.app.set_markup_color([80, 170, 255]);
                self.app.execute(Action::SelectAll, ctx);
                self.keys.push((
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::U,
                ));
            }
            5 => self.keys.push((egui::Modifiers::COMMAND, egui::Key::Z)),
            6 => self.app.execute(Action::Reload, ctx),
            7 => self.app.open_path(self.ebook.clone()),
            8 => {
                self.app.set_markup_color([255, 120, 190]);
                self.app.execute(Action::SelectAll, ctx);
                self.app.execute(Action::Highlight, ctx);
            }
            9 => self.app.open_path(sample("comic.cbr")),
            10 => self.app.execute(Action::Next, ctx),
            11 => self.app.open_path(sample("book.xps")),
            12 => self.app.open_path(sample("book.djvu")),
            13 => self.app.open_path(sample("help.chm")),
            14 => self.app.execute(Action::Home, ctx),
            15 => self.app.open_path(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("native/format-sources/libmobi/tests/samples/sample-drm-v1.mobi"),
            ),
            _ => self.finish(ctx, Ok(())),
        }
    }
    fn ready(&mut self) -> bool {
        if self.phase == 15 {
            return self.app.state().opening.is_none()
                && self
                    .app
                    .state()
                    .error
                    .as_ref()
                    .is_some_and(|error| error.kind == crabpdf::document::ErrorKind::Protected)
                && !self.app.password_prompt_active()
                && !self.app.password_retry_available();
        }
        if self.phase == 14 {
            return self.app.home_active() && self.app.recent_count() >= 6;
        }
        if self.app.state().opening.is_some() || !self.app.has_current_render() {
            return false;
        }
        match self.phase {
            0 | 7 | 11 | 12 | 13 => self.app.text_hit_position().is_some(),
            1 => self
                .app
                .selection_text()
                .is_some_and(|text| !text.trim().is_empty() && text.len() < 80),
            2 => self.app.markup_count() == 1,
            3 | 5 | 6 => self.app.markup_count() == 2,
            4 => self.app.markup_count() == 3,
            8 => self.app.markup_count() == 1,
            10 => self
                .app
                .state()
                .document
                .as_ref()
                .is_some_and(|document| document.page == 1),
            _ => true,
        }
    }
}
impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        // Control synthetic input independently of the desktop foreground window.
        // Keep screenshot replies, then inject only this scenario's events.
        input.focused = true;
        input
            .events
            .retain(|event| matches!(event, egui::Event::Screenshot { .. }));
        if let Some(position) = self.pointer.take() {
            input.events.push(egui::Event::PointerMoved(position));
            for pressed in [true, false, true, false] {
                input.events.push(egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        for (modifiers, key) in self.keys.drain(..) {
            input.events.push(egui::Event::ModifiersChanged(modifiers));
            for pressed in [true, false] {
                input.events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers,
                });
            }
            input
                .events
                .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        }
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.phase >= SHOTS.len() {
            return;
        }
        if let Some(error) = &self.app.state().error
            && self.phase != 15
        {
            self.finish(
                ctx,
                Err(format!(
                    "{}: {error} {:?}",
                    SHOTS[self.phase], error.details
                )),
            );
            return;
        }
        if self.started.elapsed() > Duration::from_secs(90) {
            self.finish(
                ctx,
                Err(format!(
                    "Timeout {}: {} marks",
                    SHOTS[self.phase],
                    self.app.markup_count()
                )),
            );
            return;
        }
        let shots: Vec<_> = ctx.input(|input| {
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
                .collect()
        });
        for shot in shots {
            if !self.capture {
                continue;
            }
            let bytes: Vec<_> = shot
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            let path = self.output.join(format!("{}.png", SHOTS[self.phase]));
            if let Err(error) = image::save_buffer(
                &path,
                &bytes,
                shot.width() as u32,
                shot.height() as u32,
                image::ColorType::Rgba8,
            ) {
                self.finish(ctx, Err(error.to_string()));
                return;
            }
            println!("Saved {}", path.display());
            self.advance(ctx);
        }
        if self.phase < SHOTS.len() && self.ready() && !self.capture {
            let settled = self.settled.get_or_insert_with(Instant::now);
            if settled.elapsed() > Duration::from_millis(300) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture = true;
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
        .unwrap_or_else(|| PathBuf::from("artifacts/formats-ui"));
    std::fs::create_dir_all(&output)?;
    // Each run starts with new documents so retained marks from a previous
    // regression run cannot alter the expected initial annotation counts.
    let run = output.join(format!(
        "inputs-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    std::fs::create_dir(&run)?;
    let source = run.join("lectura.pdf");
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reading-sample.pdf"),
        &source,
    )?;
    let ebook = run.join("libro.epub");
    std::fs::copy(sample("book.epub"), &ebook)?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF formatos",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(cc, Some(source), None),
                output,
                ebook,
                phase: 0,
                capture: false,
                settled: None,
                started: Instant::now(),
                keys: Vec::new(),
                pointer: None,
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .map_err(|_| "No se pudo leer el resultado")?
        .take()
        .ok_or("Ventana cerrada antes de terminar")?
        .map_err(Into::into)
}
