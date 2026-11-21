//! Native smoke for the dock, PDF selection, host panels and saved reading counters.
use crabpdf::{commands::Action, desktop::extensions::Repository, ui::ReaderApp};
use crabpdf_runtime::storage::profile::ProfileStore;
use eframe::egui;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

struct Smoke {
    app: ReaderApp,
    output: PathBuf,
    phase: u8,
    started: Instant,
    since: Instant,
    capture: Option<&'static str>,
    events: Vec<egui::Event>,
}

impl Smoke {
    fn step(&mut self) {
        self.phase += 1;
        self.since = Instant::now();
    }

    fn open(&mut self, key: egui::Key) {
        for pressed in [true, false] {
            self.events.push(egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::ALT.plus(egui::Modifiers::SHIFT),
            });
        }
    }

    fn pointer(&mut self, position: egui::Pos2, pressed: bool) {
        self.events.push(egui::Event::PointerMoved(position));
        self.events.push(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }

    fn screenshot(&mut self, ctx: &egui::Context, filename: &'static str) {
        self.capture = Some(filename);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    }

    fn has(&self, filename: &str) -> bool {
        self.output.join(filename).is_file()
    }
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
        if let Some(filename) = self.capture
            && let Some(image) = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                    _ => None,
                })
            })
        {
            let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            image::save_buffer(
                self.output.join(filename),
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            self.capture = None;
        }
        if self.started.elapsed() > Duration::from_secs(35) {
            eprintln!("Extension smoke stopped during phase {}", self.phase);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let ready = self.app.has_current_render() && self.app.text_hit_position().is_some();
        let settled = self.since.elapsed() > Duration::from_millis(400);
        match self.phase {
            0 if ready && settled => {
                self.app.execute(Action::Extensions, ctx);
                self.step();
            }
            1 if settled => {
                self.screenshot(ctx, "manager.png");
                self.step();
            }
            2 if self.has("manager.png") => {
                self.open(egui::Key::E);
                self.step();
            }
            3 if settled => {
                self.app.execute(Action::ExtensionArea, ctx);
                self.step();
            }
            4 if settled && ready => {
                let point = self.app.text_hit_position().unwrap() - egui::vec2(5.0, 5.0);
                self.pointer(point, true);
                self.step();
            }
            5 if settled && ready => {
                let end = self.app.text_hit_position().unwrap() + egui::vec2(270.0, 110.0);
                self.events.push(egui::Event::PointerMoved(end));
                self.step();
            }
            6 if settled && ready => {
                let point = self.app.text_hit_position().unwrap() + egui::vec2(270.0, 110.0);
                self.pointer(point, false);
                self.step();
            }
            7 if settled => {
                self.screenshot(ctx, "capture.png");
                self.step();
            }
            8 if self.has("capture.png") => {
                for pressed in [true, false] {
                    self.events.push(egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: Some(egui::Key::Enter),
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                self.step();
            }
            9 if self
                .app
                .extension_result()
                .is_some_and(|result| result.contains("Recorte")) =>
            {
                self.screenshot(ctx, "extensions.png");
                self.step();
            }
            10 if self.has("extensions.png") => {
                self.open(egui::Key::P);
                self.step();
            }
            11 if settled => {
                self.screenshot(ctx, "pomodoro.png");
                self.step();
            }
            12 if self.has("pomodoro.png") => {
                self.open(egui::Key::G);
                self.step();
            }
            13 if settled => {
                let toggle = egui::pos2(816.0, 177.0);
                self.pointer(toggle, true);
                self.pointer(toggle, false);
                self.step();
            }
            14 if settled => {
                self.screenshot(ctx, "guide.png");
                self.step();
            }
            15 if self.has("guide.png") => {
                self.open(egui::Key::T);
                self.app.execute(Action::Next, ctx);
                self.step();
            }
            16 if settled => {
                self.screenshot(ctx, "statistics.png");
                self.step();
            }
            17 if self.has("statistics.png") => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {}
        }
        ctx.request_repaint_after(Duration::from_millis(40));
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("CRABPDF_DATA_DIR").is_some() {
        return Err("Unset CRABPDF_DATA_DIR to use the isolated smoke profile".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let output = root.join(format!(
        "artifacts/extension-dock-smoke-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&output)?;
    let data = output.join("data");
    let profile = data.join("preferences.toml");
    let repository = Repository::new(&data);
    let target = crabpdf::desktop::extensions::target();
    for source in [
        root.join("artifacts/extensions/selection-info"),
        root.join(format!("artifacts/extension-build/pomodoro-{target}")),
        root.join(format!("artifacts/extension-build/reading-guide-{target}")),
        root.join(format!("artifacts/extension-build/reading-stats-{target}")),
    ] {
        let manifest = repository.inspect(&source)?;
        repository.install(&source)?;
        let package = repository
            .list()?
            .into_iter()
            .find(|package| package.manifest.id == manifest.id)
            .expect("installed package is in the inventory");
        repository.enable(&package, true)?;
    }
    std::fs::write(
        data.join("extension-shortcuts.json"),
        br#"{
        "org.crabpdf.selection-info": ["E", null],
        "org.crabpdf.pomodoro": ["P", null],
        "org.crabpdf.reading-guide": ["G", null],
        "org.crabpdf.reading-stats": ["T", null]
    }"#,
    )?;
    let document = output.join("sample.pdf");
    std::fs::copy(root.join("tests/fixtures/reading-sample.pdf"), &document)?;
    let report = output.clone();
    let config = profile.clone();
    eframe::run_native(
        "CrabPDF extension integration",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(cc, Some(document), Some(config)),
                output: report,
                phase: 0,
                started: Instant::now(),
                since: Instant::now(),
                capture: None,
                events: Vec::new(),
            }))
        }),
    )?;
    println!("{}", output.display());
    for name in [
        "extensions.png",
        "pomodoro.png",
        "guide.png",
        "statistics.png",
    ] {
        if !output.join(name).is_file() {
            return Err(format!("Missing snapshot {name}").into());
        }
    }
    let totals = ProfileStore::load(&profile)?.reading_totals;
    if totals.page_visits < 2 || totals.milliseconds < 1_000 {
        return Err(format!("Reading counters did not persist: {totals:?}").into());
    }
    Ok(())
}
