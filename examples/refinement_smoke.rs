//! Native regression of library, vertical reading, keyboard zoom and window modes.
use crabpdf::{
    commands::Action,
    document::{PageColors, Zoom},
    ui::ReaderApp,
};
use eframe::egui;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const SHOTS: [&str; 46] = [
    "01-vertical-ready",
    "02-vertical-scroll",
    "03-zoom-anchor",
    "04-library",
    "05-home-to-document",
    "06-palette-filter",
    "07-palette-arrow-enter",
    "08-control-wheel",
    "09-fullscreen-document",
    "10-minimal-reading",
    "11-reading-to-fullscreen",
    "12-back-to-window",
    "13-maximized",
    "14-restored",
    "15-100-percent",
    "16-125-percent-dpi125",
    "17-150-percent-dpi150",
    "18-200-percent-dpi200",
    "19-history-removal",
    "20-many-tabs",
    "21-keyboard-plus",
    "22-percent-button-focus",
    "23-right-percent-page",
    "24-left-percent-page",
    "25-fit-height",
    "26-right-fit-height",
    "27-left-fit-height",
    "28-wide-page",
    "29-right-pan",
    "30-left-pan",
    "31-search-focus",
    "32-search-cursor-left",
    "33-search-insert-at-cursor",
    "34-search-dismissed",
    "35-fullscreen-clean",
    "36-fullscreen-overlay",
    "37-fullscreen-overlay-hidden",
    "38-fullscreen-restored",
    "39-presentation-from-window",
    "40-f11-exits-presentation",
    "41-fullscreen-keyboard",
    "42-fullscreen-right-page",
    "43-fullscreen-left-page",
    "44-fullscreen-sidebar",
    "45-fullscreen-sidebar-hidden",
    "46-window-sidebar-restored",
];

struct Smoke {
    app: ReaderApp,
    phase: usize,
    output: PathBuf,
    capture: bool,
    settled: Option<Instant>,
    started: Instant,
    keys: Vec<(egui::Modifiers, egui::Key)>,
    text: Option<String>,
    wheel: bool,
    anchor: Option<(f32, f32)>,
    wheel_before: u16,
    mode_zoom: Zoom,
    pointer: Option<egui::Pos2>,
    pan_before: f32,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        if let Ok(mut outcome) = self.outcome.lock() {
            *outcome = Some(result);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn key(&mut self, key: egui::Key) {
        self.keys.push((egui::Modifiers::NONE, key));
    }
    fn advance(&mut self, ctx: &egui::Context) {
        // A timer can run logic without painting in egui 0.36. Every new
        // scene must invalidate the UI, including zooms already in the cache.
        ctx.request_repaint();
        self.phase += 1;
        self.capture = false;
        self.settled = None;
        self.started = Instant::now();
        match self.phase {
            1 => self.app.execute(Action::ScrollDownPage, ctx),
            2 => {
                self.anchor = self.app.current_page_viewport().map(|bounds| {
                    (
                        (bounds.left + bounds.right) * 0.5,
                        (bounds.top + bounds.bottom) * 0.5,
                    )
                });
                self.app.set_zoom(Zoom::Percent(200));
            }
            3 => {
                self.app.open_path(fixture("features-sample.pdf"));
                self.app.execute(Action::Home, ctx);
            }
            4 => self.keys.push((
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND,
                egui::Key::Tab,
            )),
            5 => {
                self.app.execute(Action::Palette, ctx);
                self.text = Some("zoom".into());
            }
            6 => {
                self.key(egui::Key::ArrowDown);
                self.key(egui::Key::Enter);
            }
            7 => {
                self.wheel_before = if let Zoom::Percent(p) = self.app.state().zoom {
                    p
                } else {
                    0
                };
                self.wheel = true;
            }
            8 => {
                self.mode_zoom = self.app.state().zoom;
                self.app.execute(Action::Outline, ctx);
                self.key(egui::Key::F11);
            }
            9 => self.key(egui::Key::F5),
            10 | 11 => self.key(egui::Key::Escape),
            12 | 13 => self.app.execute(Action::Maximize, ctx),
            14 => {
                ctx.set_zoom_factor(1.0);
                self.app.set_zoom(Zoom::Percent(100));
            }
            15 => {
                ctx.set_zoom_factor(1.25);
                self.app.set_zoom(Zoom::Percent(125));
            }
            16 => {
                ctx.set_zoom_factor(1.5);
                self.app.set_zoom(Zoom::Percent(150));
            }
            17 => {
                ctx.set_zoom_factor(2.0);
                self.app.set_zoom(Zoom::Percent(200));
            }
            18 => {
                ctx.set_zoom_factor(1.0);
                self.app.forget_recent(&fixture("features-sample.pdf"));
                self.app.execute(Action::Home, ctx);
            }
            19 => {
                self.app.execute(Action::NextTab, ctx);
            }
            20 => self.keys.push((
                egui::Modifiers::CTRL | egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Equals,
            )),
            21 => {
                self.app.execute(Action::First, ctx);
                self.app.set_zoom(Zoom::Percent(50));
                ctx.memory_mut(|memory| {
                    if let Some(id) = memory.focused() {
                        memory.surrender_focus(id);
                    }
                });
                self.key(egui::Key::Tab);
            }
            22 | 25 | 28 => {
                if self.phase == 28 {
                    self.pan_before = self.app.current_page_viewport().unwrap().left;
                }
                self.key(egui::Key::ArrowRight);
            }
            23 | 26 | 29 | 31 => self.key(egui::Key::ArrowLeft),
            24 => self.app.set_zoom(Zoom::FitHeight),
            27 => {
                self.app.set_zoom(Zoom::Percent(200));
                self.app.execute(Action::First, ctx);
            }
            30 => {
                self.app.execute(Action::Find, ctx);
                self.text = Some("reader".into());
            }
            32 => self.text = Some("X".into()),
            33 => self.key(egui::Key::Escape),
            34 | 40 => {
                self.app.execute(Action::VerticalReading, ctx);
                self.key(egui::Key::F11);
                if self.phase == 40 {
                    self.key(egui::Key::F6);
                }
            }
            37 | 39 => self.key(egui::Key::F11),
            45 => {
                self.key(egui::Key::F11);
                self.key(egui::Key::F6);
            }
            35 => self.pointer = Some(egui::pos2(ctx.viewport_rect().center().x, 1.0)),
            36 => self.pointer = Some(ctx.viewport_rect().center()),
            38 => self.key(egui::Key::F5),
            41 => self.key(egui::Key::ArrowRight),
            42 => self.key(egui::Key::ArrowLeft),
            43 | 44 => self.key(egui::Key::F12),
            _ => self.finish(ctx, Ok(())),
        }
    }
    fn ready(&self, ctx: &egui::Context) -> Result<bool, String> {
        if let Some(error) = &self.app.state().error {
            return Err(error.to_string());
        }
        let rendered = self.app.has_current_render();
        let page = self
            .app
            .state()
            .document
            .as_ref()
            .map(|document| document.page);
        let full_canvas = self.app.reading_viewport().is_some_and(|rect| {
            let screen = ctx.viewport_rect();
            (rect.min - screen.min).length() < 1.0 && (rect.max - screen.max).length() < 1.0
        });
        let ready = match self.phase {
            0 => rendered && self.app.prefetch_ready(1),
            1 => rendered,
            2 => {
                if !rendered {
                    return Ok(false);
                }
                let Some(before) = self.anchor else {
                    return Err("No se obtuvo el ancla de lectura.".into());
                };
                let Some(bounds) = self.app.current_page_viewport() else {
                    return Ok(false);
                };
                let after = (
                    (bounds.left + bounds.right) * 0.5,
                    (bounds.top + bounds.bottom) * 0.5,
                );
                if (before.0 - after.0).abs() > 0.015 || (before.1 - after.1).abs() > 0.015 {
                    return Err(format!("El zoom movió el centro: {before:?} -> {after:?}"));
                }
                true
            }
            3 => {
                self.app.home_active()
                    && self.app.tab_count() == 2
                    && self.app.library_cover_ready(&fixture("reading-sample.pdf"))
                    && self
                        .app
                        .library_cover_ready(&fixture("features-sample.pdf"))
            }
            4 => {
                rendered
                    && !self.app.home_active()
                    && self
                        .app
                        .state()
                        .document
                        .as_ref()
                        .is_some_and(|d| d.info.file_name() == "reading-sample.pdf")
            }
            5 => rendered,
            6 => rendered && self.app.state().zoom == Zoom::Percent(150),
            7 => rendered && self.app.state().zoom != Zoom::Percent(self.wheel_before),
            8 | 10 => {
                rendered
                    && self.app.screen_modes() == (true, false)
                    && ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
                    && full_canvas
            }
            9 => {
                rendered
                    && self.app.screen_modes() == (true, true)
                    && self.app.state().zoom == self.mode_zoom
                    && full_canvas
            }
            11 => {
                rendered
                    && self.app.screen_modes() == (false, false)
                    && self.app.state().zoom == self.mode_zoom
                    && !ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
            }
            12 => rendered && ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
            13 => rendered && !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
            14..=17 => rendered,
            18 => {
                self.app.home_active()
                    && self.app.tab_count() == 2
                    && self.app.recent_count() == 1
                    && fixture("features-sample.pdf").exists()
            }
            19 => rendered && self.app.tab_count() == 10,
            20 => {
                rendered
                    && self.app.state().zoom == Zoom::Percent(300)
                    && (ctx.zoom_factor() - 1.0).abs() < 0.001
            }
            21 => {
                rendered
                    && page == Some(0)
                    && ctx.egui_wants_keyboard_input()
                    && !ctx.text_edit_focused()
            }
            22 | 25 => rendered && page == Some(1),
            23 | 24 | 26 | 27 => rendered && page == Some(0),
            28 => {
                rendered
                    && page == Some(0)
                    && self
                        .app
                        .current_page_viewport()
                        .is_some_and(|bounds| bounds.left > self.pan_before + 0.005)
            }
            29 => {
                rendered
                    && page == Some(0)
                    && self
                        .app
                        .current_page_viewport()
                        .is_some_and(|bounds| (bounds.left - self.pan_before).abs() < 0.002)
            }
            30 | 31 => {
                rendered
                    && ctx.text_edit_focused()
                    && self.app.search_query() == Some("reader")
                    && page == Some(0)
            }
            32 => rendered && self.app.search_query() == Some("readeXr") && page == Some(0),
            33 => rendered && !ctx.egui_wants_keyboard_input(),
            34 | 36 | 40 | 44 => {
                rendered
                    && full_canvas
                    && self.app.screen_modes() == (true, false)
                    && self.app.fullscreen_controls_rect().is_none()
                    && (self.phase != 40 || self.app.state().colors == PageColors::Inverted)
                    && self
                        .app
                        .current_page_viewport()
                        .is_some_and(|bounds| bounds.right - bounds.left < 1.02)
            }
            35 => rendered && full_canvas && self.app.fullscreen_controls_rect().is_some(),
            37 | 39 | 45 => {
                rendered
                    && (self.phase != 45 || self.app.state().colors == PageColors::Original)
                    && self.app.screen_modes() == (false, false)
                    && !ctx.input(|i| i.viewport().fullscreen.unwrap_or(false))
                    && self.app.reading_viewport().is_some_and(|rect| {
                        rect.top() > 60.0 && rect.width() < ctx.viewport_rect().width() - 150.0
                    })
            }
            38 => rendered && full_canvas && self.app.screen_modes() == (true, true),
            41 | 42 => {
                rendered
                    && full_canvas
                    && self.app.screen_modes() == (true, false)
                    && page == Some(usize::from(self.phase == 41))
            }
            43 => {
                rendered
                    && self.app.screen_modes() == (true, false)
                    && self.app.reading_viewport().is_some_and(|rect| {
                        rect.top() < 1.0 && rect.width() < ctx.viewport_rect().width() - 150.0
                    })
            }
            _ => false,
        };
        Ok(ready)
    }
}
impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        // Control synthetic input independently of the desktop foreground window.
        // Keep screenshot replies, then inject only this scenario's events.
        input.focused = true;
        input
            .events
            .retain(|event| matches!(event, egui::Event::Screenshot { .. }));
        if let Some(pointer) = self.pointer.take() {
            input.events.push(egui::Event::PointerMoved(pointer));
        }
        if let Some(text) = self.text.take() {
            input.events.push(egui::Event::Text(text));
        }
        for (modifiers, key) in self.keys.drain(..) {
            for pressed in [true, false] {
                input.events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers,
                });
            }
        }
        if self.wheel {
            self.wheel = false;
            let modifiers = egui::Modifiers::CTRL | egui::Modifiers::COMMAND;
            input.events.push(egui::Event::ModifiersChanged(modifiers));
            input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(500.0, 500.0)));
            input.events.push(egui::Event::MouseWheel {
                phase: egui::TouchPhase::Move,
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, 1.0),
                modifiers,
            });
            input
                .events
                .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        }
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.phase == 19 && self.app.tab_count() < 10 && self.app.has_current_render() {
            self.app.execute(Action::DuplicateTab, ctx);
        }
        if self.started.elapsed() > Duration::from_secs(60) {
            self.finish(
                ctx,
                Err(format!(
                    "Timeout en {}: zoom {:?}, modos {:?}",
                    SHOTS[self.phase],
                    self.app.state().zoom,
                    self.app.screen_modes()
                )),
            );
            return;
        }
        let shots = ctx.input(|input| {
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
        for image in shots {
            if !self.capture || self.phase >= SHOTS.len() {
                continue;
            }
            let path = self.output.join(format!("{}.png", SHOTS[self.phase]));
            let pixels: Vec<_> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
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
            self.advance(ctx);
        }
        if self.phase >= SHOTS.len() {
            return;
        }
        match self.ready(ctx) {
            Err(error) => self.finish(ctx, Err(error)),
            Ok(true)
                if !self.capture
                    && self.settled.get_or_insert_with(Instant::now).elapsed()
                        > Duration::from_millis(300) =>
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                self.capture = true;
            }
            _ => {}
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
        .unwrap_or_else(|| PathBuf::from("artifacts/refinement/ui"));
    std::fs::create_dir_all(&output)?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF refinement test",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_config(cc, Some(fixture("reading-sample.pdf")), None),
                phase: 0,
                output,
                capture: false,
                settled: None,
                started: Instant::now(),
                keys: Vec::new(),
                text: None,
                wheel: false,
                anchor: None,
                wheel_before: 0,
                mode_zoom: Zoom::FitWidth,
                pointer: None,
                pan_before: 0.0,
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .map_err(|_| "Resultado bloqueado")?
        .take()
        .ok_or("Recorrido incompleto")?
        .map_err(Into::into)
}
