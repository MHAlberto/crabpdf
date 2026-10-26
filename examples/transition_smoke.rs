//! Native, frame-by-frame regression for keyboard page transitions and reader chrome.
use crabpdf::{
    commands::Action,
    config::{Preferences, Theme},
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
    reduced: bool,
    phase: usize,
    began: Instant,
    ready_since: Option<Instant>,
    capture: bool,
    finish_shot: bool,
    shot: usize,
    keys: Vec<egui::Key>,
    pointer: Option<(egui::Pos2, bool)>,
    previous_y: Option<f32>,
    offsets: Vec<(usize, f32)>,
    reversed: bool,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.outcome.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn advance(&mut self, ctx: &egui::Context) {
        self.phase += 1;
        self.began = Instant::now();
        self.ready_since = None;
        self.shot = 0;
        self.finish_shot = false;
        self.previous_y = self.app.scroll_offset().map(|p| p.y);
        match self.phase {
            1 => self.keys.push(egui::Key::ArrowRight),
            2 => self.keys.push(egui::Key::ArrowLeft),
            3 => self.keys.push(egui::Key::ArrowRight),
            4 => {
                self.app.execute(Action::Continuous, ctx);
                self.keys.push(egui::Key::ArrowRight);
            }
            5 => {
                self.app.execute(Action::First, ctx);
                self.app.execute(Action::SelectAll, ctx);
            }
            6 => {
                self.app.set_markup_color([90, 210, 100]);
                if let Some(rect) = self.app.markup_button_rect() {
                    self.pointer = Some((rect.center(), true));
                } else {
                    self.finish(ctx, Err("Falta botón Marcas".into()));
                }
            }
            7 => {
                egui::Popup::close_all(ctx);
                self.app.execute(Action::Highlight, ctx);
            }
            8 => self.app.execute(Action::Home, ctx),
            9 => self.app.execute(Action::NextTab, ctx),
            10 => self.app.execute(Action::Settings, ctx),
            11 => {
                self.keys.push(egui::Key::Escape);
                self.pointer = Some((egui::pos2(ctx.viewport_rect().right() - 28.0, 18.0), false));
            }
            12 => {
                let trace = serde_json::to_vec_pretty(&self.offsets).unwrap();
                if let Err(error) = std::fs::write(self.output.join("offsets.json"), trace) {
                    self.finish(ctx, Err(error.to_string()));
                    return;
                }
                let next: Vec<_> = self
                    .offsets
                    .iter()
                    .filter(|(phase, _)| *phase == 1)
                    .map(|(_, y)| *y)
                    .collect();
                let min = next.iter().copied().fold(f32::INFINITY, f32::min);
                let max = next.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let intermediate = next
                    .iter()
                    .filter(|y| **y > min + 5.0 && **y < max - 5.0)
                    .count();
                if max - min < 100.0
                    || (!self.reduced && intermediate == 0)
                    || (self.reduced && intermediate > 0)
                {
                    self.finish(
                        ctx,
                        Err(format!(
                            "Transición incorrecta: reduced={}, {next:?}",
                            self.reduced
                        )),
                    );
                    return;
                }
                *self.outcome.lock().unwrap() = Some(Ok(()));
                self.pointer = Some((egui::pos2(ctx.viewport_rect().right() - 28.0, 18.0), true));
            }
            _ => {}
        }
    }
    fn ready(&mut self) -> bool {
        let page = self.app.state().document.as_ref().map(|d| d.page);
        let settled = self.app.has_current_render() && self.app.current_page_viewport().is_some();
        match self.phase {
            0 => settled && self.app.prefetch_ready(1),
            1 => settled && page == Some(1),
            2 => settled && page == Some(0),
            3 => settled && self.reversed && page == Some(0),
            4 => settled && !self.app.state().continuous && page == Some(1),
            5 => settled && self.app.selection_text().is_some(),
            6 | 10 | 11 => self.began.elapsed() > Duration::from_millis(350),
            7 => settled && self.app.markup_count() == 1,
            8 => self.app.state().document.is_none(),
            9 => settled,
            _ => false,
        }
    }
}
impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        for key in self.keys.drain(..) {
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
        if let Some((pos, click)) = self.pointer.take() {
            input.events.push(egui::Event::PointerMoved(pos));
            if click {
                for pressed in [true, false] {
                    input.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
        }
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.began.elapsed() > Duration::from_secs(if self.phase == 12 { 4 } else { 60 }) {
            self.finish(ctx, Err(format!("Timeout en escena {}", self.phase)));
            return;
        }
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
            return;
        }
        if self.phase == 3 && !self.reversed && self.began.elapsed() > Duration::from_millis(55) {
            self.keys.push(egui::Key::ArrowLeft);
            self.reversed = true;
        }
        let screenshots: Vec<_> = ctx.input(|input| {
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
        for image in screenshots {
            if !self.capture {
                continue;
            }
            if matches!(self.phase, 1 | 2 | 4)
                && let Some(rect) = self.app.reading_viewport()
            {
                let scale = image.width() as f32 / ctx.viewport_rect().width();
                let x = (rect.center().x * scale) as usize;
                let top = ((rect.top() + 20.0) * scale) as usize;
                let bottom = ((rect.bottom() - 20.0) * scale).min(image.height() as f32) as usize;
                let white = (top..bottom)
                    .filter(|y| {
                        let pixel = image[(x.min(image.width() - 1), *y)];
                        pixel.r() > 245 && pixel.g() > 245 && pixel.b() > 245
                    })
                    .count();
                if bottom > top && white * 10 > bottom - top {
                    self.finish(
                        ctx,
                        Err(format!(
                            "Destello blanco en escena {}, frame {}",
                            self.phase, self.shot
                        )),
                    );
                    return;
                }
            }
            let pixels: Vec<_> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            let path = self
                .output
                .join(format!("{:02}-{:02}.png", self.phase, self.shot));
            if let Err(error) = image::save_buffer(
                &path,
                &pixels,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                self.finish(ctx, Err(error.to_string()));
                return;
            }
            println!("Saved {}", path.display());
            self.capture = false;
            self.shot += 1;
            if self.finish_shot {
                self.advance(ctx);
                return;
            }
        }
        if self.phase >= 12 {
            return;
        }
        if self.ready() {
            self.ready_since.get_or_insert_with(Instant::now);
        } else {
            self.ready_since = None;
        }
        let ready = self
            .ready_since
            .is_some_and(|since| since.elapsed() > Duration::from_millis(170));
        if !self.capture && (ready || (matches!(self.phase, 1 | 2 | 4) && self.shot < 10)) {
            self.capture = true;
            self.finish_shot = ready;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        ctx.request_repaint_after(Duration::from_millis(10));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if self.phase == 0
            && let Some(rect) = self.app.reading_viewport()
            && (rect.bottom() - ui.ctx().viewport_rect().bottom()).abs() > 2.0
        {
            self.finish(
                ui.ctx(),
                Err("La barra inferior sigue ocupando espacio".into()),
            );
        }
        if matches!(self.phase, 1 | 2)
            && let Some(offset) = self.app.scroll_offset()
        {
            if let Some(previous) = self.previous_y
                && ((self.phase == 1 && offset.y + 0.5 < previous)
                    || (self.phase == 2 && offset.y > previous + 0.5))
            {
                self.finish(
                    ui.ctx(),
                    Err(format!("Rebote de página: {previous} -> {}", offset.y)),
                );
                return;
            }
            self.previous_y = Some(offset.y);
            self.offsets.push((self.phase, offset.y));
        }
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}

fn write_fixture(path: &std::path::Path) -> std::io::Result<()> {
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Count 3 /Kids [3 0 R 5 0 R 7 0 R] >>".to_string(),
    ];
    for (index, color) in ["0.8 0.2 0.2", "0.15 0.4 0.8", "0.2 0.65 0.35"]
        .iter()
        .enumerate()
    {
        let stream = format!(
            "{color} rg 0 0 600 800 re f 1 1 1 rg BT /F1 24 Tf 40 740 Td (CrabPDF Page {}) Tj ET\n",
            index + 1
        );
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 600 800] /Resources << /Font << /F1 9 0 R >> >> /Contents {} 0 R >>", index * 2 + 4));
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}endstream",
            stream.len(),
            stream
        ));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    let mut pdf = b"%PDF-1.4\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).as_bytes());
    for offset in &offsets[1..] {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .as_bytes(),
    );
    std::fs::write(path, pdf)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let output = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/transition-smoke"));
    let reduced = args.iter().any(|arg| arg == "--reduced");
    let dark = args.iter().any(|arg| arg == "--dark");
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let fixture = output.join("colored-pages.pdf");
    write_fixture(&fixture)?;
    let config = output.join("preferences.toml");
    crabpdf::config::ProfileStore::save(
        &Preferences {
            reduce_motion: reduced,
            theme: if dark { Theme::Dark } else { Theme::Light },
            ..Default::default()
        },
        &config,
    )?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF transitions",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([900.0, 760.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(cc, Some(fixture), Some(config)),
                output,
                reduced,
                phase: 0,
                began: Instant::now(),
                ready_since: None,
                capture: false,
                finish_shot: false,
                shot: 0,
                keys: Vec::new(),
                pointer: None,
                previous_y: None,
                offsets: Vec::new(),
                reversed: false,
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .map_err(|_| "Resultado bloqueado")?
        .take()
        .ok_or("Recorrido interrumpido")?
        .map_err(Into::into)
}
