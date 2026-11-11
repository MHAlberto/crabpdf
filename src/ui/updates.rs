use crate::desktop::updates::{self, Installed, Package, Update};
use eframe::egui;
use std::{sync::mpsc, thread};

#[derive(Default)]
pub(super) struct UpdateDialog {
    open: bool,
    status: Status,
}

#[derive(Default)]
enum Status {
    #[default]
    Idle,
    Working(&'static str, mpsc::Receiver<Result<Status, String>>),
    Current,
    Available(Update),
    Installed,
    ClosingForInstaller,
    Unsupported,
    Error(String),
}

impl UpdateDialog {
    pub fn open(&mut self, ctx: &egui::Context) {
        self.open = true;
        if matches!(
            self.status,
            Status::Idle | Status::Current | Status::Error(_)
        ) {
            self.check(ctx);
        }
    }

    fn check(&mut self, ctx: &egui::Context) {
        let Some(package) = Package::detect() else {
            self.status = Status::Unsupported;
            return;
        };
        self.start(ctx, "Consultando las versiones de GitHub…", move || {
            updates::check(package).map(|update| update.map_or(Status::Current, Status::Available))
        });
    }

    fn start(
        &mut self,
        ctx: &egui::Context,
        label: &'static str,
        work: impl FnOnce() -> Result<Status, String> + Send + 'static,
    ) {
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        let spawn = thread::Builder::new()
            .name("crabpdf-update".into())
            .spawn(move || {
                let _ = sender.send(work());
                ctx.request_repaint();
            });
        self.status = match spawn {
            Ok(_) => Status::Working(label, receiver),
            Err(error) => Status::Error(format!("No se pudo iniciar la actualización. {error}")),
        };
    }

    pub fn poll(&mut self, ctx: &egui::Context) {
        let Status::Working(_, receiver) = &self.status else {
            return;
        };
        self.status = match receiver.try_recv() {
            Ok(Ok(status)) => status,
            Ok(Err(error)) => Status::Error(error),
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Status::Error("Se interrumpió la actualización.".into())
            }
        };
        if matches!(self.status, Status::ClosingForInstaller) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        let mut open = true;
        let mut install = false;
        let mut retry = false;
        egui::Window::new("Actualizaciones de CrabPDF")
            .collapsible(false)
            .resizable(false)
            .default_width(395.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label(concat!("Versión en uso: ", env!("CARGO_PKG_VERSION")));
                ui.add_space(6.0);
                match &self.status {
                    Status::Working(label, _) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(*label);
                        });
                    }
                    Status::Current => { ui.label("No hay una versión más reciente para este paquete."); }
                    Status::Available(update) => {
                        ui.label(format!("CrabPDF {} está disponible.", update.version));
                        ui.hyperlink_to("Ver notas de la versión", update.notes_url());
                        if update.package.can_install() {
                            ui.label(match update.package {
                                Package::Debian => "La instalación conserva tus datos y pide autorización de administrador.",
                                _ => "CrabPDF guardará tu sesión y se cerrará al abrir el instalador.",
                            });
                            install = ui.button("Descargar e instalar").clicked();
                        } else {
                            ui.label("Extrae la nueva versión en una carpeta aparte y ábrela desde allí.");
                            ui.hyperlink_to("Descargar paquete portable", update.download_url());
                        }
                    }
                    Status::Installed => {
                        ui.label("Actualización instalada. Cierra y vuelve a abrir CrabPDF para usarla.");
                        if ui.button("Cerrar CrabPDF").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                    Status::Error(error) => {
                        ui.label(error);
                        retry = ui.button("Volver a buscar").clicked();
                        ui.hyperlink_to("Abrir descargas de GitHub", updates::RELEASES);
                    }
                    Status::Unsupported => {
                        ui.label("Consulta los paquetes disponibles para tu plataforma.");
                        ui.hyperlink_to("Abrir descargas de GitHub", updates::RELEASES);
                    }
                    Status::Idle | Status::ClosingForInstaller => {}
                }
            });
        self.open = open;
        if install && let Status::Available(update) = std::mem::take(&mut self.status) {
            self.start(ctx, "Descargando, verificando e instalando…", move || {
                updates::install(&update).map(|installed| match installed {
                    Installed::WindowsInstaller => Status::ClosingForInstaller,
                    Installed::DebianPackage => Status::Installed,
                })
            });
        }
        if retry {
            self.check(ctx);
        }
    }
}
