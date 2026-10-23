//! Native regression for book favorites, tab drag duplication and fullscreen origins.
use crabpdf::{
    commands::Action,
    config::{Favorite, Preferences},
    document::Zoom,
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
    phase: usize,
    began: Instant,
    settled: Option<Instant>,
    capture: bool,
    events: Vec<egui::Event>,
    source: Option<egui::Rect>,
    native: bool,
    source_anchor: Option<(f32, f32)>,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name)
        .canonicalize()
        .unwrap()
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.outcome.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn pointer(&mut self, pos: egui::Pos2, pressed: Option<bool>) {
        self.events.push(egui::Event::PointerMoved(pos));
        if let Some(pressed) = pressed {
            self.events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
    }
    fn click(&mut self, rect: Option<egui::Rect>) {
        let rect = rect.expect("Falta el botón del recorrido");
        self.pointer(rect.center(), Some(true));
        self.pointer(rect.center(), Some(false));
    }
    fn advance(&mut self, ctx: &egui::Context) {
        if self.phase == 4 {
            self.source_anchor = self
                .app
                .current_page_viewport()
                .map(|b| ((b.left + b.right) * 0.5, (b.top + b.bottom) * 0.5));
        }
        self.phase += 1;
        self.began = Instant::now();
        self.settled = None;
        self.capture = false;
        let inside = self.source.map(|r| r.center()).unwrap_or_default();
        let outside = inside + egui::vec2(0.0, 100.0);
        match self.phase {
            1 => {
                self.app.set_zoom(Zoom::Percent(125));
                self.app.execute(Action::Favorites, ctx);
            }
            2 => self.click(self.app.favorite_button_rect(1)),
            3 | 4 => self.click(self.app.favorite_toggle_rect()),
            5 => self.app.open_path(fixture("features-sample.pdf")),
            6 | 11 | 15 => {
                self.source = self.app.tab_button_rect(0);
                self.pointer(
                    self.source.expect("Falta pestaña de origen").center(),
                    Some(true),
                );
            }
            7 | 12 | 16 => self.pointer(outside, None),
            8 | 17 => self.pointer(outside, Some(false)),
            9 | 10 => self.app.execute(Action::PreviousTab, ctx),
            13 => self.pointer(inside, None),
            14 => self.pointer(inside, Some(false)),
            18 => {
                self.app.execute(Action::ToggleSidebar, ctx);
                self.app.set_zoom(Zoom::FitWidth);
                self.app.execute(Action::Fullscreen, ctx);
            }
            19 => {}
            20 | 22 | 23 => self.app.execute(Action::Fullscreen, ctx),
            21 | 24 => self.app.execute(Action::Maximize, ctx),
            25 => self.finish(ctx, Ok(())),
            _ => {}
        }
        if self.phase == 16 {
            for pressed in [true, false] {
                self.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: Some(egui::Key::Escape),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
    }
    fn ready(&self, ctx: &egui::Context) -> Result<bool, String> {
        let loaded = self.app.has_current_render() && self.app.current_page_viewport().is_some();
        let page = self.app.state().document.as_ref().map(|d| d.page);
        let favorites = self.app.current_favorites();
        let a = fixture("reading-sample.pdf");
        let b = fixture("features-sample.pdf");
        let book = self.app.state().document.as_ref().map(|d| &d.info.path);
        let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if self.phase == 1
            && loaded
            && (favorites.len() != 2 || favorites.iter().any(|f| f.path != a))
        {
            return Err("Se mezclaron favoritos de otros libros".into());
        }
        if matches!(self.phase, 8 | 10)
            && book == Some(&a)
            && page == Some(1)
            && loaded
            && let (Some(expected), Some(bounds)) =
                (self.source_anchor, self.app.current_page_viewport())
        {
            let actual = (
                (bounds.left + bounds.right) * 0.5,
                (bounds.top + bounds.bottom) * 0.5,
            );
            if (expected.0 - actual.0).abs() > 0.04 || (expected.1 - actual.1).abs() > 0.04 {
                return Err(format!(
                    "La duplicación movió la vista: {expected:?} -> {actual:?}"
                ));
            }
        }
        let correct = match self.phase {
            0 => loaded,
            1 => loaded && self.app.favorite_button_rect(1).is_some(),
            2 => {
                loaded
                    && page == Some(1)
                    && self.app.state().zoom == Zoom::Percent(125)
                    && self.app.state().continuous
            }
            3 => favorites.len() == 1 && page == Some(1),
            4 => favorites.len() == 2,
            5 => {
                loaded
                    && book == Some(&b)
                    && favorites.len() == 1
                    && favorites[0].label == "Solo libro B"
            }
            6 | 7 | 11 | 12 | 13 | 15 | 16 => self.began.elapsed() > Duration::from_millis(120),
            8 => {
                loaded
                    && self.app.tab_count() == 3
                    && book == Some(&a)
                    && page == Some(1)
                    && self.app.state().zoom == Zoom::Percent(125)
            }
            9 => loaded && book == Some(&b) && favorites.len() == 1,
            10 => {
                loaded
                    && book == Some(&a)
                    && page == Some(1)
                    && favorites.len() == 2
                    && self.app.state().zoom == Zoom::Percent(125)
            }
            14 | 17 => self.app.tab_count() == 3,
            18 | 19 | 22 => {
                loaded
                    && full
                    && self.app.reading_viewport().is_some_and(|r| {
                        let screen = ctx.viewport_rect();
                        (r.min - screen.min).length() < 1.0 && (r.max - screen.max).length() < 1.0
                    })
            }
            20 => loaded && !full,
            21 => loaded && maximized,
            23 => loaded && !full && maximized,
            24 => loaded && !full && !maximized,
            _ => false,
        };
        Ok(correct)
    }
}
impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        input.events.append(&mut self.events);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.phase >= 25 {
            return;
        }
        if self.began.elapsed() > Duration::from_secs(30) {
            self.finish(
                ctx,
                Err(format!(
                    "Timeout phase {}: page {:?}, tabs {}, zoom {:?}",
                    self.phase,
                    self.app.state().document.as_ref().map(|d| d.page),
                    self.app.tab_count(),
                    self.app.state().zoom
                )),
            );
            return;
        }
        let shots = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Screenshot { image, .. } = e {
                        Some(Arc::clone(image))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        for image in shots {
            if !self.capture {
                continue;
            }
            let pixels: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            let path = self.output.join(format!("{:02}.png", self.phase));
            if let Err(e) = image::save_buffer(
                &path,
                &pixels,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            ) {
                self.finish(ctx, Err(e.to_string()));
                return;
            }
            println!("Saved {}", path.display());
            self.advance(ctx);
        }
        match self.ready(ctx) {
            Err(e) => self.finish(ctx, Err(e)),
            Ok(true) if !self.capture => {
                let delay = if matches!(self.phase, 18 | 19 | 22) {
                    800
                } else {
                    180
                };
                if self.settled.get_or_insert_with(Instant::now).elapsed()
                    > Duration::from_millis(delay)
                {
                    let info = serde_json::json!({"phase":self.phase,"native":self.native,"fullscreen":ctx.input(|i|i.viewport().fullscreen),"maximized":ctx.input(|i|i.viewport().maximized)});
                    if let Err(e) =
                        std::fs::write(self.output.join("ready-phase.json"), info.to_string())
                    {
                        self.finish(ctx, Err(e.to_string()));
                        return;
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    self.capture = true;
                }
            }
            _ => {}
        }
        ctx.request_repaint_after(Duration::from_millis(20));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        self.app.on_exit(gl);
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let output = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artifacts/document-chrome"));
    let native = args.iter().any(|a| a == "--native");
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let config = output.join("preferences.toml");
    let a = fixture("reading-sample.pdf");
    let b = fixture("features-sample.pdf");
    Preferences {
        native_titlebar: native,
        favorites: vec![
            Favorite {
                path: a.clone(),
                page: 1,
                label: "Página 2".into(),
            },
            Favorite {
                path: b,
                page: 0,
                label: "Solo libro B".into(),
            },
            Favorite {
                path: a.clone(),
                page: 0,
                label: "reading-sample.pdf · 1".into(),
            },
        ],
        ..Default::default()
    }
    .save(&config)?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF document chrome",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(cc, Some(a), Some(config)),
                output,
                phase: 0,
                began: Instant::now(),
                settled: None,
                capture: false,
                events: Vec::new(),
                source: None,
                native,
                source_anchor: None,
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
