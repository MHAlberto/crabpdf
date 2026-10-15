//! Three independent native runs: save a position, restore at another size/scale, then override via CLI.
use crabpdf::commands::Action;
use crabpdf::config::Preferences;
use crabpdf::ui::ReaderApp;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

struct Smoke {
    app: ReaderApp,
    stage: String,
    output: PathBuf,
    phase: usize,
    started: Instant,
    settled: Option<Instant>,
    capture: bool,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}
impl Smoke {
    fn finish(&self, ctx: &egui::Context, result: Result<(), String>) {
        *self.result.lock().unwrap() = Some(result);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}
impl eframe::App for Smoke {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.started.elapsed() > Duration::from_secs(60) {
            self.finish(
                ctx,
                Err(format!("Timeout {} phase {}", self.stage, self.phase)),
            );
            return;
        }
        if let Some(error) = &self.app.state().error {
            self.finish(ctx, Err(error.to_string()));
            return;
        }
        let screenshot = ctx.input(|input| {
            input.raw.events.iter().find_map(|event| {
                if let egui::Event::Screenshot { image, .. } = event {
                    Some(Arc::clone(image))
                } else {
                    None
                }
            })
        });
        if self.capture
            && let Some(image) = screenshot
        {
            let pixels: Vec<_> = image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_array())
                .collect();
            let path = self.output.join(format!("{}.png", self.stage));
            let result = image::save_buffer(
                &path,
                &pixels,
                image.width() as u32,
                image.height() as u32,
                image::ColorType::Rgba8,
            )
            .map_err(|e| e.to_string());
            println!("Saved {}", path.display());
            self.finish(ctx, result);
            return;
        }
        if !self.capture
            && self.app.has_current_render()
            && let Some(view) = self.app.current_page_viewport()
        {
            let ready = self.settled.get_or_insert_with(Instant::now).elapsed()
                > Duration::from_millis(350);
            if ready {
                if self.stage == "write" && self.phase == 0 {
                    let (hits, scanned, total) = self.app.search_progress();
                    if hits < 2 || scanned != total {
                        ctx.request_repaint_after(Duration::from_millis(30));
                        return;
                    }
                    self.app.execute(Action::Manga, ctx);
                    for _ in 1..hits {
                        self.app.execute(Action::FindNext, ctx);
                    }
                    self.phase = 2;
                    self.settled = None;
                } else {
                    let point = (
                        (view.left + view.right) * 0.5,
                        (view.top + view.bottom) * 0.5,
                    );
                    let page = self.app.state().document.as_ref().unwrap().page;
                    let result = if self.stage == "write" {
                        if page != 1 || point.1 < 0.2 {
                            Err(format!("Fixture no desplazado: {page} {point:?}"))
                        } else {
                            std::fs::write(
                                self.output.join("expected.json"),
                                serde_json::to_vec(&point).unwrap(),
                            )
                            .map_err(|e| e.to_string())
                        }
                    } else if self.stage == "read" {
                        let expected: (f32, f32) = serde_json::from_slice(
                            &std::fs::read(self.output.join("expected.json")).unwrap(),
                        )
                        .unwrap();
                        if page != 1
                            || !self.app.state().manga
                            || (point.0 - expected.0).abs() > 0.02
                            || (point.1 - expected.1).abs() > 0.02
                        {
                            Err(format!(
                                "No se recuperó el punto: página {page}, {point:?} vs {expected:?}, manga={}",
                                self.app.state().manga
                            ))
                        } else {
                            Ok(())
                        }
                    } else if page != 0 {
                        Err("--page no prevaleció sobre el punto guardado".into())
                    } else {
                        Ok(())
                    };
                    if let Err(error) = result {
                        self.finish(ctx, Err(error));
                        return;
                    }
                    self.capture = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                }
            }
        } else if !self.capture {
            self.settled = None;
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
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_none_or(|arg| arg != "--stage") {
        let output = args.first().map(PathBuf::from).unwrap_or_else(|| {
            root.join(format!(
                "artifacts/reading-session-{}-{}",
                std::env::consts::ARCH,
                std::process::id()
            ))
        });
        std::fs::create_dir_all(&output)?;
        let output = output.canonicalize()?;
        Preferences {
            restore_session: true,
            ..Preferences::default()
        }
        .save(&output.join("preferences.toml"))?;
        for stage in ["write", "read", "cli"] {
            let status = std::process::Command::new(std::env::current_exe()?)
                .arg("--stage")
                .arg(stage)
                .arg(&output)
                .status()?;
            if !status.success() {
                return Err(format!("Falló lectura persistente: {stage}").into());
            }
            if stage == "write" {
                let prefs = Preferences::load(&output.join("preferences.toml"))?;
                if !prefs
                    .session
                    .first()
                    .is_some_and(|saved| saved.manga && saved.position.is_some())
                {
                    return Err("El cierre no persistió el punto y manga".into());
                }
            }
        }
        println!("Reading session: three independent processes passed.");
        return Ok(());
    }
    let stage = args
        .get(1)
        .ok_or("Falta etapa")?
        .to_string_lossy()
        .into_owned();
    let output = PathBuf::from(args.get(2).ok_or("Falta salida")?);
    let config = output.join("preferences.toml");
    let mut launch = crabpdf::cli::LaunchOptions {
        config: Some(config),
        ..Default::default()
    };
    if stage == "write" {
        launch.files.push(root.join("examples/tall-sample.pdf"));
        launch.view.search = Some("reader".into());
    }
    if stage == "cli" {
        let crabpdf::cli::Invocation::Run(parsed) = crabpdf::cli::parse([
            "--config".into(),
            output.join("preferences.toml").into_os_string(),
            "--page".into(),
            "1".into(),
            "--zoom".into(),
            "fit-page".into(),
            root.join("examples/tall-sample.pdf").into_os_string(),
        ])?
        else {
            return Err("CLI inesperada".into());
        };
        launch = parsed;
    }
    let result = Arc::new(Mutex::new(None));
    let outcome = Arc::clone(&result);
    let size = if stage == "read" {
        [820.0, 620.0]
    } else {
        [1080.0, 780.0]
    };
    eframe::run_native(
        "CrabPDF reading session",
        eframe::NativeOptions {
            viewport: crabpdf::branding::viewport().with_inner_size(size),
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        },
        Box::new(move |cc| {
            if stage == "read" {
                cc.egui_ctx.set_zoom_factor(1.25);
            }
            Ok(Box::new(Smoke {
                app: ReaderApp::with_launch(cc, launch),
                stage,
                output,
                phase: 0,
                started: Instant::now(),
                settled: None,
                capture: false,
                result,
            }))
        }),
    )?;
    outcome
        .lock()
        .map_err(|_| "Resultado bloqueado")?
        .take()
        .ok_or("Prueba interrumpida")?
        .map_err(Into::into)
}
