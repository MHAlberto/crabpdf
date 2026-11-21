//! Native notebook flow with a private PDF and profile.
use crabpdf::{commands::Action, ui::ReaderApp};
use crabpdf_runtime::storage::notebook;
use eframe::egui;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

struct Smoke {
    app: ReaderApp,
    report: PathBuf,
    data: PathBuf,
    key: notebook::NotebookKey,
    started: Instant,
    settled: Instant,
    phase: u8,
    inject: bool,
}

impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        input.focused = true;
        if std::mem::take(&mut self.inject) {
            for pressed in [true, false] {
                input.events.push(egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: Some(egui::Key::Enter),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                });
            }
        }
    }

    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.started.elapsed() > Duration::from_secs(25) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        match self.phase {
            0 if self.app.has_current_render() && self.app.text_hit_position().is_some() => {
                self.app.execute(Action::SelectAll, ctx);
                self.phase = 1;
            }
            1 if self
                .app
                .selection_text()
                .is_some_and(|text| text.contains("reader")) =>
            {
                self.app.execute(Action::NotebookQuote, ctx);
                self.settled = Instant::now();
                self.phase = 2;
            }
            2 if self.settled.elapsed() > Duration::from_millis(250) => {
                if notebook::load(&self.data, &self.key).is_ok_and(|notes| !notes.is_empty()) {
                    self.settled = Instant::now();
                    self.phase = 3;
                } else {
                    self.inject = true;
                    self.settled = Instant::now();
                }
            }
            3 if self.settled.elapsed() > Duration::from_millis(300) => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.phase = 4;
            }
            _ => {}
        }
        if self.phase == 4
            && let Some(image) = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                    _ => None,
                })
            })
        {
            let pixels: Vec<_> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            image::save_buffer(
                self.report.join("notebook.png"),
                &pixels,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint_after(Duration::from_millis(30));
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
    let report = root.join(format!("artifacts/notebook-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&report)?;
    let source = report.join("source.pdf");
    let data = report.join("data");
    std::fs::copy(root.join("tests/fixtures/reading-sample.pdf"), &source)?;
    let key = notebook::identify(&source)?;
    let run_data = data.clone();
    let run_report = report.clone();
    let run_source = source.clone();
    let options = eframe::NativeOptions {
        viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "CrabPDF notebook smoke",
        options,
        Box::new(move |cc| {
            let app = ReaderApp::with_config(
                cc,
                Some(run_source),
                Some(run_data.join("preferences.toml")),
            );
            Ok(Box::new(Smoke {
                app,
                report: run_report,
                data: run_data,
                key,
                started: Instant::now(),
                settled: Instant::now(),
                phase: 0,
                inject: false,
            }))
        }),
    )?;
    if !report.join("notebook.png").is_file() {
        return Err("The notebook UI did not save and render a quote".into());
    }
    let moved = report.join("moved.pdf");
    std::fs::rename(&source, &moved)?;
    let notes = notebook::load(&data, &notebook::identify(&moved)?)?;
    if notes.len() != 1 || !notes[0].excerpt.contains("reader") || notes[0].anchor.region.is_none()
    {
        return Err("The saved quote or its region was lost after moving the PDF".into());
    }
    println!("{}", report.display());
    Ok(())
}
