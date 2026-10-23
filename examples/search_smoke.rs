//! Native regression: reveal a tall-page hit, clear the query, then replace it rapidly.
use crabpdf::cli::{self, Invocation};
use crabpdf::commands::Action;
use crabpdf::ui::ReaderApp;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct SearchSmoke {
    app: ReaderApp,
    advanced: bool,
    phase: u8,
    changed: Instant,
    events: Vec<egui::Event>,
    started: Instant,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
}
impl SearchSmoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.outcome.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn replace_query(&mut self, query: &str) {
        for (key, modifiers) in [
            (
                egui::Key::A,
                egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            ),
            (egui::Key::Backspace, egui::Modifiers::NONE),
        ] {
            for pressed in [true, false] {
                self.events.push(egui::Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers,
                });
            }
        }
        if !query.is_empty() {
            self.events.push(egui::Event::Text(query.into()));
        }
    }
}
impl eframe::App for SearchSmoke {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        input.events.append(&mut self.events);
    }
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.started.elapsed() > Duration::from_secs(35) {
            self.finish(ctx, Err(format!("Timeout search phase {}", self.phase)));
            return;
        }
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
            return;
        }
        let (hits, scanned, total) = self.app.search_progress();
        match self.phase {
            0 => {
                if !self.advanced && hits >= 6 && scanned == total {
                    for _ in 1..hits {
                        self.app.execute(Action::FindNext, ctx);
                    }
                    self.advanced = true;
                }
                if self.advanced
                    && self.app.has_current_render()
                    && let Some(viewport) = self.app.current_page_viewport()
                {
                    if !self.app.current_search_hit().is_some_and(|hit| {
                        hit.page == 1
                            && hit.bounds.iter().any(|bounds| bounds.intersects(&viewport))
                    }) {
                        self.finish(
                            ctx,
                            Err("Tall-page search result is outside the viewport.".into()),
                        );
                        return;
                    }
                    self.app.execute(Action::Find, ctx);
                    self.phase = 1;
                    self.changed = Instant::now();
                }
            }
            1 if self.changed.elapsed() > Duration::from_millis(100) => {
                self.replace_query("");
                self.phase = 2;
                self.changed = Instant::now();
            }
            2 if self.app.search_query() == Some("")
                && self.changed.elapsed() > Duration::from_millis(350) =>
            {
                if hits != 0 {
                    self.finish(ctx, Err("Cleared query retained search results.".into()));
                    return;
                }
                self.replace_query("never-matches-this-document");
                self.phase = 3;
                self.changed = Instant::now();
            }
            3 if self.app.search_query() == Some("never-matches-this-document")
                && self.changed.elapsed() > Duration::from_millis(50) =>
            {
                self.replace_query("reader");
                self.phase = 4;
            }
            4 if self.app.search_query() == Some("reader") && hits >= 6 && scanned == total => {
                if self
                    .app
                    .current_search_hit()
                    .is_some_and(|hit| hit.page == 0)
                {
                    println!(
                        "Search UI: tall-page reveal, empty query and rapid replacement passed."
                    );
                    self.finish(ctx, Ok(()));
                } else {
                    self.finish(
                        ctx,
                        Err("The latest query did not reveal its first result.".into()),
                    );
                }
            }
            _ => {}
        }
        ctx.request_repaint_after(Duration::from_millis(15));
    }
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args = vec![
        "--search".into(),
        "reader".into(),
        "--zoom".into(),
        "fit-width".into(),
        "--view".into(),
        "single".into(),
        "--rotate".into(),
        "0".into(),
        "--config".into(),
        root.join(format!(
            "artifacts/search-smoke-{}.toml",
            std::env::consts::ARCH
        ))
        .into_os_string(),
        root.join("examples/tall-sample.pdf").into_os_string(),
    ];
    let Invocation::Run(launch) = cli::parse(args)? else {
        return Err("Arranque inválido".into());
    };
    let outcome = Arc::new(Mutex::new(None));
    let result = Arc::clone(&outcome);
    eframe::run_native(
        "CrabPDF search regression",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size([1080.0, 780.0]),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            Ok(Box::new(SearchSmoke {
                app: ReaderApp::with_launch(cc, launch),
                advanced: false,
                phase: 0,
                changed: Instant::now(),
                events: Vec::new(),
                started: Instant::now(),
                outcome,
            }))
        }),
    )?;
    result
        .lock()
        .map_err(|_| "Error de resultado")?
        .take()
        .ok_or("Prueba interrumpida")?
        .map_err(Into::into)
}
