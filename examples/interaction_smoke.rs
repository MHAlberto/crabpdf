//! Native regression: no blank marking frames and continuous arrow motion without OS repeat.
use crabpdf::{
    commands::Action,
    document::{Bounds, Glyph, PageContent, Zoom},
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
    started: Instant,
    events: Vec<egui::Event>,
    capture: bool,
    baseline: usize,
    shots: usize,
    rows: Vec<serde_json::Value>,
    scroll: Vec<serde_json::Value>,
    last_render_ms: Option<f64>,
    anchor: f32,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.outcome.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn key(&mut self, key: egui::Key, pressed: bool) {
        self.events.push(egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    fn enter(&mut self, phase: usize, ctx: &egui::Context) {
        self.phase = phase;
        self.started = Instant::now();
        self.shots = 0;
        self.last_render_ms = None;
        // Synthetic events need a wake just like native input; do not wait for
        // a worker response to deliver the first held-key frame.
        ctx.request_repaint();
        match phase {
            1 => {
                let pos = self.app.text_hit_position().unwrap();
                self.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true, false, true, false] {
                    self.events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            2 => self.app.execute(Action::Highlight, ctx),
            3 => {
                self.app.execute(Action::SelectAll, ctx);
                self.app.execute(Action::Underline, ctx);
            }
            4 => self.app.execute(Action::UndoMarkup, ctx),
            5 => {
                self.app.execute(Action::VerticalReading, ctx);
            }
            6 => {
                self.anchor = self.app.scroll_offset().unwrap().y;
                self.key(egui::Key::ArrowDown, true);
            }
            7 => self.key(egui::Key::ArrowDown, false),
            8 => self.key(egui::Key::ArrowUp, true),
            9 => self.key(egui::Key::ArrowUp, false),
            _ => {}
        }
    }
    fn dark_text(&self, image: &egui::ColorImage, ctx: &egui::Context) -> usize {
        let view = self.app.reading_viewport().unwrap();
        let scale = image.width() as f32 / ctx.viewport_rect().width();
        let region =
            egui::Rect::from_center_size(view.center(), view.size() * egui::vec2(0.36, 0.55));
        let mut count = 0;
        for y in (region.top() * scale) as usize
            ..((region.bottom() * scale) as usize).min(image.height())
        {
            for x in (region.left() * scale) as usize
                ..((region.right() * scale) as usize).min(image.width())
            {
                let pixel = image[(x, y)];
                // Local contrast detects ink through blue selections and yellow marks;
                // a fixed RGB threshold mistakes those tints for disappearing text.
                let neighbor = image[((x + 2).min(image.width() - 1), y)];
                let light =
                    |p: egui::Color32| u16::from(p.r()) + u16::from(p.g()) + u16::from(p.b());
                if light(neighbor).saturating_sub(light(pixel)) > 70 {
                    count += 1;
                }
            }
        }
        count
    }
}
impl eframe::App for Smoke {
    fn raw_input_hook(&mut self, _: &egui::Context, input: &mut egui::RawInput) {
        input.events.append(&mut self.events);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.started.elapsed() > Duration::from_secs(20) {
            self.finish(ctx, Err(format!("Timeout phase {}", self.phase)));
            return;
        }
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64() * 1000.0;
        let images = ctx.input(|input| {
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
        for image in images {
            self.capture = false;
            let dark = self.dark_text(&image, ctx);
            let bytes: Vec<_> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            image::save_buffer(
                self.output
                    .join(format!("phase{}-{:03}.png", self.phase, self.shots)),
                &bytes,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .unwrap();
            self.rows.push(serde_json::json!({"phase":self.phase,"elapsed_ms":elapsed,"text_edge_pixels":dark}));
            self.shots += 1;
            if self.phase == 1 {
                self.baseline = dark;
                if dark < 300 {
                    self.finish(ctx, Err("Baseline text region missing".into()));
                    return;
                }
                self.enter(2, ctx);
            } else if (2..=4).contains(&self.phase) && dark < self.baseline / 3 {
                self.finish(
                    ctx,
                    Err(format!(
                        "Blank marking frame: phase {} pixels {dark}, baseline {}",
                        self.phase, self.baseline
                    )),
                );
                return;
            }
        }
        if (2..=4).contains(&self.phase)
            && self.app.has_current_render()
            && self.app.markup_count() == if self.phase == 3 { 2 } else { 1 }
            && self.last_render_ms.is_none()
        {
            self.last_render_ms = Some(elapsed);
            self.rows
                .push(serde_json::json!({"phase":self.phase,"current_render_ms":elapsed}));
        }
        match self.phase {
            0 if self.app.has_current_render() && self.app.text_hit_position().is_some() => {
                self.enter(1, ctx)
            }
            1 if self.app.selection_text().is_some() && !self.capture => {
                self.rows
                    .push(serde_json::json!({"word_selection_ms":elapsed}));
                self.capture = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            2..=4 => {
                if elapsed > 450.0 && self.shots >= 5 && self.last_render_ms.is_some() {
                    self.enter(self.phase + 1, ctx);
                } else if !self.capture {
                    self.capture = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            }
            5 if self.app.has_current_render() && elapsed > 300.0 => self.enter(6, ctx),
            6 if elapsed > 700.0 => {
                let delta = self.app.scroll_offset().unwrap().y - self.anchor;
                if delta < 250.0 {
                    self.finish(ctx, Err(format!("Held down stalled: delta {delta}")));
                    return;
                }
                self.enter(7, ctx);
            }
            7 if elapsed > 220.0 => self.enter(8, ctx),
            8 if elapsed > 700.0 => self.enter(9, ctx),
            9 if elapsed > 220.0 => {
                let report = serde_json::json!({"profile":if cfg!(debug_assertions) {"dev"} else {"release"},
                    "mark_frames":self.rows,"scroll_frames":self.scroll,"baseline_text_edge_pixels":self.baseline,
                    "limits":"Local native fixture; screenshots add overhead. These are regression observations, not global latency guarantees."});
                std::fs::write(
                    self.output.join("interaction-report.json"),
                    serde_json::to_vec_pretty(&report).unwrap(),
                )
                .unwrap();
                self.finish(ctx, Ok(()));
                return;
            }
            _ => {}
        }
        // During holds the app itself must schedule every animation frame.
        if !(6..=9).contains(&self.phase) {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_secs(2));
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let began = Instant::now();
        self.app.ui(ui, frame);
        if (6..=9).contains(&self.phase) {
            let y = self.app.scroll_offset().unwrap().y;
            if let Some(last) = self.scroll.last() {
                let same = last["phase"].as_u64() == Some(self.phase as u64);
                let old = last["y"].as_f64().unwrap() as f32;
                if same
                    && ((self.phase == 6 && y < old - 0.1) || (self.phase == 8 && y > old + 0.1))
                {
                    self.finish(ui.ctx(), Err("Arrow motion reversed unexpectedly".into()));
                }
            }
            self.scroll.push(serde_json::json!({"phase":self.phase,"elapsed_ms":self.started.elapsed().as_secs_f64()*1000.0,
                "y":y,"ui_ms":began.elapsed().as_secs_f64()*1000.0}));
        }
    }
}
fn hit_benchmark(output: &std::path::Path) {
    let content = PageContent {
        glyphs: (0..6000)
            .map(|index| {
                let x = (index % 100) as f32 * 0.009;
                let y = (index / 100) as f32 * 0.015;
                Glyph {
                    index,
                    text: "a".into(),
                    bounds: Some(Bounds {
                        left: x,
                        right: x + 0.008,
                        top: y,
                        bottom: y + 0.012,
                    }),
                }
            })
            .collect(),
        ..Default::default()
    };
    let reference = |x: f32, y: f32| {
        content
            .glyphs
            .iter()
            .filter_map(|g| g.bounds.map(|b| (g.index, b)))
            .min_by(|(_, a), (_, b)| {
                let distance = |r: Bounds| {
                    (x.clamp(r.left, r.right) - x).powi(2) + (y.clamp(r.top, r.bottom) - y).powi(2)
                };
                distance(*a).total_cmp(&distance(*b))
            })
            .map(|(index, _)| index)
    };
    let queries: Vec<_> = (0..512)
        .map(|step| {
            (
                ((step * 37) % 110) as f32 / 100.0,
                ((step * 17) % 105) as f32 / 100.0,
            )
        })
        .collect();
    for &(x, y) in &queries {
        assert_eq!(reference(x, y), content.nearest_glyph(x, y));
    }
    let mut rounds = Vec::new();
    for round in 0..8 {
        let mut times = [0.0; 2];
        let order = if round % 2 == 0 { [0, 1] } else { [1, 0] };
        for variant in order {
            let now = Instant::now();
            for &(x, y) in &queries {
                std::hint::black_box(if variant == 0 {
                    reference(std::hint::black_box(x), y)
                } else {
                    content.nearest_glyph(std::hint::black_box(x), y)
                });
            }
            times[variant] = now.elapsed().as_secs_f64() * 1_000_000.0 / queries.len() as f64;
        }
        rounds.push(serde_json::json!({"reference_us_per_query":times[0],"single_pass_us_per_query":times[1]}));
    }
    std::fs::write(output.join("hit-benchmark.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "glyphs":6000,"queries_per_round":512,"rounds":rounds,
        "profile":if cfg!(debug_assertions) {"dev"} else {"release"},
        "limits":"Synthetic local hit-testing microbenchmark; not whole-app or PDF parsing speed."
    })).unwrap()).unwrap();
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| "artifacts/interaction-fixes/native".into());
    std::fs::create_dir_all(&output)?;
    hit_benchmark(&output);
    let source = output.join(format!("reading-{}.pdf", std::process::id()));
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/reading-sample.pdf"),
        &source,
    )?;
    let config = output.join("preferences.toml");
    crabpdf::config::Preferences::default().save(&config)?;
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF interaction regression",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(Smoke {
                app: ReaderApp::with_launch(
                    cc,
                    crabpdf::cli::LaunchOptions {
                        files: vec![source],
                        config: Some(config),
                        view: crabpdf::cli::InitialView {
                            zoom: Some(Zoom::FitPage),
                            continuous: Some(false),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ),
                output,
                phase: 0,
                started: Instant::now(),
                events: Vec::new(),
                capture: false,
                baseline: 0,
                shots: 0,
                rows: Vec::new(),
                scroll: Vec::new(),
                last_render_ms: None,
                anchor: 0.0,
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .unwrap()
        .take()
        .ok_or("Interrupted interaction smoke")?
        .map_err(Into::into)
}
