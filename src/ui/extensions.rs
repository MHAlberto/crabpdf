//! Extension manager and user-selected page regions.
use super::*;
use crate::desktop::extensions::{InstalledPackage, Invocation, Repository, RunningExtension};
use crabpdf_core::document::{Bounds, SelectionCapture};
use crabpdf_extension_api::{ExtensionCommand, ExtensionOutput, InputKind, Region};
use eframe::egui::{Color32, Rect, Stroke, StrokeKind};
use std::{path::PathBuf, sync::mpsc};

enum ManagementResult {
    Packages(Vec<InstalledPackage>),
    Preview(PathBuf, Box<crabpdf_extension_api::Manifest>),
}
#[derive(Default)]
pub(super) struct Extensions {
    pub open: bool,
    packages: Vec<InstalledPackage>,
    operation: Option<mpsc::Receiver<Result<ManagementResult, String>>>,
    picker: Option<crate::desktop::dialogs::PendingDialog>,
    preview: Option<(PathBuf, crabpdf_extension_api::Manifest)>,
    pub selecting: bool,
    drag: Option<(DocumentId, usize, egui::Pos2)>,
    selected: Option<(DocumentId, usize, Bounds)>,
    capture: Option<SelectionCapture>,
    pending: Option<(DocumentId, u64, u64)>,
    serial: u64,
    revision: Option<u64>,
    running: Option<RunningExtension>,
    output: Option<ExtensionOutput>,
    message: Option<String>,
}
impl Extensions {
    pub fn result_text(&self) -> Option<&str> {
        self.output.as_ref().map(|output| output.text.as_str())
    }
    fn busy(&self) -> bool {
        self.operation.is_some() || self.running.is_some() || self.pending.is_some()
    }
    fn manage(
        &mut self,
        ctx: &egui::Context,
        work: impl FnOnce() -> std::io::Result<ManagementResult> + Send + 'static,
    ) {
        let (send, recv) = mpsc::channel();
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("crabpdf-packages".into())
            .spawn(move || {
                let _ = send.send(work().map_err(|e| e.to_string()));
                ctx.request_repaint();
            }) {
            Ok(_) => self.operation = Some(recv),
            Err(e) => self.message = Some(e.to_string()),
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, repository: Repository, can_select: bool) -> bool {
        let mut arm = false;
        if !self.open {
            return false;
        }
        let mut open = true;
        egui::Window::new("Extensiones")
            .default_width(540.0)
            .default_height(450.0)
            .open(&mut open)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(560.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.busy(),
                                    egui::Button::new("Instalar desde carpeta…"),
                                )
                                .clicked()
                            {
                                self.picker = Some(
                                    crate::desktop::dialogs::PendingDialog::extension_folder(),
                                );
                            }
                            if ui
                                .add_enabled(!self.busy(), egui::Button::new("Actualizar lista"))
                                .clicked()
                            {
                                let repository = repository.clone();
                                self.manage(ctx, move || {
                                    repository.list().map(ManagementResult::Packages)
                                });
                            }
                        });
                        if let Some((path, manifest)) = self.preview.clone() {
                            ui.separator();
                            ui.strong(format!("{} {}", manifest.name, manifest.version));
                            ui.label(&manifest.description);
                            ui.label(
                                "El paquete nativo se ejecuta con los permisos de tu usuario.",
                            );
                            for command in &manifest.commands {
                                ui.label(format!(
                                    "{} · entrada {:?} · salida {:?}",
                                    command.title, command.input, command.output
                                ));
                            }
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        !self.busy(),
                                        egui::Button::new("Instalar paquete"),
                                    )
                                    .clicked()
                                {
                                    self.preview = None;
                                    let repository = repository.clone();
                                    self.manage(ctx, move || {
                                        repository.install(&path)?;
                                        repository.list().map(ManagementResult::Packages)
                                    });
                                }
                                if ui.button("Descartar").clicked() {
                                    self.preview = None;
                                }
                            });
                        }
                        if self.packages.is_empty() {
                            ui.label("Instala un paquete para añadir herramientas al lector.");
                        }
                        for package in self.packages.clone() {
                            ui.separator();
                            ui.strong(format!(
                                "{} {}",
                                package.manifest.name, package.manifest.version
                            ));
                            ui.label(&package.manifest.description);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        !self.busy(),
                                        egui::Button::new(if package.enabled {
                                            "Desactivar"
                                        } else {
                                            "Activar"
                                        }),
                                    )
                                    .clicked()
                                {
                                    let package = package.clone();
                                    let repository = repository.clone();
                                    self.manage(ctx, move || {
                                        repository.enable(&package, !package.enabled)?;
                                        repository.list().map(ManagementResult::Packages)
                                    });
                                }
                                if ui
                                    .add_enabled(!self.busy(), egui::Button::new("Desinstalar"))
                                    .clicked()
                                {
                                    let package = package.clone();
                                    let repository = repository.clone();
                                    self.manage(ctx, move || {
                                        repository.remove(&package)?;
                                        repository.list().map(ManagementResult::Packages)
                                    });
                                }
                            });
                            if package.enabled {
                                for command in &package.manifest.commands {
                                    let ready = !self.busy()
                                        && self.capture.as_ref().is_some_and(|capture| {
                                            command.input == InputKind::Region
                                                || !capture.text.trim().is_empty()
                                        });
                                    if ui
                                        .add_enabled(ready, egui::Button::new(&command.title))
                                        .clicked()
                                    {
                                        self.run(
                                            ctx,
                                            repository.clone(),
                                            package.clone(),
                                            command.clone(),
                                        );
                                    }
                                }
                            }
                        }
                        ui.separator();
                        if let (Some((_, page, _)), Some(capture)) =
                            (self.selected, self.capture.as_ref())
                        {
                            ui.label(format!(
                                "Área de la página {} · {} × {} px",
                                page + 1,
                                capture.width,
                                capture.height
                            ));
                        }
                        if ui
                            .add_enabled(
                                can_select && !self.busy(),
                                egui::Button::new("Seleccionar área del PDF…"),
                            )
                            .clicked()
                        {
                            self.selecting = true;
                            self.drag = None;
                            self.capture = None;
                            self.selected = None;
                            self.open = false;
                            arm = true;
                        }
                        ui.label("También puedes usar Texto / Enviar selección a extensión.");
                        if self.pending.is_some() {
                            ui.label("Preparando el recorte…");
                        }
                        if self.operation.is_some() {
                            ui.spinner();
                        }
                        if self.running.is_some() {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Ejecutando extensión…");
                                if ui.button("Cancelar").clicked() {
                                    if let Some(run) = self.running.take() {
                                        run.cancel();
                                    }
                                    self.message = Some("Operación cancelada.".into());
                                }
                            });
                        }
                        if let Some(message) = &self.message {
                            ui.label(message);
                        }
                        if let Some(output) = &mut self.output {
                            ui.separator();
                            ui.horizontal(|ui| {
                                ui.label(match output.kind {
                                    crabpdf_extension_api::OutputKind::Text => "Resultado · Texto",
                                    crabpdf_extension_api::OutputKind::Latex => "Resultado · LaTeX",
                                });
                                if ui.button("Copiar resultado").clicked() {
                                    ctx.copy_text(output.text.clone());
                                }
                            });
                            ui.add(
                                egui::TextEdit::multiline(&mut output.text)
                                    .code_editor()
                                    .desired_rows(8)
                                    .desired_width(f32::INFINITY),
                            );
                        }
                    });
            });
        self.open = open && !arm;
        arm
    }
    fn run(
        &mut self,
        ctx: &egui::Context,
        repository: Repository,
        package: InstalledPackage,
        command: ExtensionCommand,
    ) {
        let (Some((_, page, bounds)), Some(capture)) = (self.selected, self.capture.as_ref())
        else {
            return;
        };
        let input = Invocation {
            command,
            page: page + 1,
            region: Region {
                left: bounds.left,
                top: bounds.top,
                right: bounds.right,
                bottom: bounds.bottom,
            },
            text: capture.text.clone(),
            image: Some((capture.png.clone(), capture.width, capture.height)),
        };
        let repaint = ctx.clone();
        match RunningExtension::spawn(repository, package, input, move || {
            repaint.request_repaint()
        }) {
            Ok(run) => {
                self.running = Some(run);
                self.output = None;
                self.message = None;
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }
    pub fn cancel_selection(&mut self) -> bool {
        let selecting = self.selecting;
        self.selecting = false;
        self.drag = None;
        selecting
    }
}
impl ReaderApp {
    pub(super) fn start_extension_area(&mut self, ctx: &egui::Context) {
        if self.extensions.busy() {
            return;
        }
        self.extensions_open(ctx);
        self.extensions.selecting = true;
        self.extensions.open = false;
        self.extensions.drag = None;
        self.extensions.capture = None;
        self.extensions.output = None;
        self.extensions.selected = None;
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.view.selection_drag = None;
        }
    }
    pub(super) fn extensions_open(&mut self, ctx: &egui::Context) {
        self.extensions.open = true;
        if self.extensions.busy() {
            return;
        }
        let repository = Repository::new(&self.app_paths.data);
        self.extensions.manage(ctx, move || {
            repository.list().map(ManagementResult::Packages)
        });
    }
    pub(super) fn poll_extensions(&mut self, ctx: &egui::Context) {
        let identity = self
            .tabs
            .get(self.active)
            .filter(|_| !self.home)
            .and_then(|tab| {
                tab.reader
                    .state
                    .document
                    .as_ref()
                    .map(|_| (tab.reader.id, tab.reader.state.content_revision))
            });
        if self
            .extensions
            .selected
            .is_some_and(|(id, _, _)| identity.is_none_or(|(active, _)| id != active))
            || self
                .extensions
                .revision
                .is_some_and(|revision| identity.is_none_or(|(_, current)| current != revision))
            || self
                .extensions
                .pending
                .is_some_and(|(id, _, revision)| identity != Some((id, revision)))
        {
            self.extensions.capture = None;
            self.extensions.pending = None;
            self.extensions.selected = None;
            self.extensions.running = None;
            self.extensions.output = None;
        }
        if let Some(receiver) = &self.extensions.operation {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Se interrumpió la operación de paquetes.".into()))
                }
            };
            if let Some(result) = result {
                self.extensions.operation = None;
                match result {
                    Ok(ManagementResult::Packages(packages)) => {
                        self.extensions.packages = packages;
                        self.extensions.message = None;
                    }
                    Ok(ManagementResult::Preview(path, manifest)) => {
                        self.extensions.preview = Some((path, *manifest))
                    }
                    Err(error) => self.extensions.message = Some(error),
                }
            }
        }
        if let Some(picker) = &mut self.extensions.picker
            && let Some((_, files)) = picker.poll(ctx)
        {
            self.extensions.picker = None;
            if let Some(path) = files.into_iter().next() {
                let repository = Repository::new(&self.app_paths.data);
                self.extensions.manage(ctx, move || {
                    repository
                        .inspect(&path)
                        .map(|manifest| ManagementResult::Preview(path, Box::new(manifest)))
                });
            }
        }
        if let Some(run) = &self.extensions.running
            && let Some(result) = run.poll()
        {
            self.extensions.running = None;
            match result {
                Ok(output) => self.extensions.output = Some(output),
                Err(e) => self.extensions.message = Some(e),
            }
        }
        if self.extensions.selecting && self.notice.is_none() {
            self.notice=Some("Arrastra un rectángulo sobre la ecuación o el área que quieras enviar. Escape cancela.".into());
        }
    }
    pub(super) fn capture_for_extension(
        &mut self,
        page: usize,
        bounds: Bounds,
        ctx: &egui::Context,
    ) {
        if self.extensions.pending.is_some() || self.extensions.running.is_some() {
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        if !tab
            .reader
            .state
            .details
            .as_ref()
            .is_some_and(|details| details.can_copy)
        {
            return;
        }
        self.extensions.revision = Some(tab.reader.state.content_revision);
        self.extensions.serial = self.extensions.serial.wrapping_add(1);
        let token = self.extensions.serial;
        self.extensions.selected = Some((tab.reader.id, page, bounds));
        self.extensions.capture = None;
        self.extensions.output = None;
        self.notice = None;
        self.extensions.selecting = false;
        self.extensions.drag = None;
        self.extensions.open = true;
        if let Some(worker) = &self.worker {
            match worker.capture(
                tab.reader.id,
                token,
                tab.reader.state.content_revision,
                page,
                bounds,
            ) {
                Ok(()) => {
                    self.extensions.pending =
                        Some((tab.reader.id, token, tab.reader.state.content_revision))
                }
                Err(e) => self.extensions.message = Some(e.to_string()),
            }
        }
        ctx.request_repaint();
    }
    pub(super) fn accept_extension_capture(
        &mut self,
        id: DocumentId,
        token: u64,
        result: Result<SelectionCapture, ReaderError>,
    ) {
        if self
            .extensions
            .pending
            .is_none_or(|(requested, t, _)| (requested, t) != (id, token))
        {
            return;
        }
        self.extensions.pending = None;
        match result {
            Ok(capture) => {
                self.extensions.capture = Some(capture);
                self.extensions.message = None;
            }
            Err(error) => self.extensions.message = Some(error.to_string()),
        }
    }
    pub(super) fn extension_text_selection(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let Some(selection) = tab
            .reader
            .selection
            .range()
            .and_then(|range| range.single_page())
        else {
            self.notice =
                Some("Selecciona texto en una página o elige un área desde Extensiones.".into());
            return;
        };
        let id = tab.reader.id;
        let Some(content) = self.need_content(id, selection.page) else {
            self.notice = Some("Espera a que termine de cargar el texto.".into());
            return;
        };
        let mut bounds: Option<Bounds> = None;
        for glyph in content
            .glyphs
            .iter()
            .filter(|glyph| selection.contains(glyph.index))
        {
            if let Some(b) = glyph.bounds {
                bounds = Some(bounds.map_or(b, |a| Bounds {
                    left: a.left.min(b.left),
                    top: a.top.min(b.top),
                    right: a.right.max(b.right),
                    bottom: a.bottom.max(b.bottom),
                }));
            }
        }
        if let Some(bounds) = bounds {
            self.extensions_open(ctx);
            self.capture_for_extension(selection.page, bounds, ctx);
        }
    }
    pub(super) fn extension_page_interaction(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        page: usize,
        rotation: u8,
    ) -> bool {
        if !self.extensions.selecting {
            return false;
        }
        let id = self.tabs[self.active].reader.id;
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(start) = ui.input(|i| i.pointer.press_origin())
        {
            self.extensions.drag = Some((id, page, start));
        }
        if let Some((document, anchor_page, start)) = self.extensions.drag
            && document == id
            && anchor_page == page
            && let Some(end) = ui.input(|i| i.pointer.interact_pos())
        {
            let rect_selection = Rect::from_two_pos(rect.clamp(start), rect.clamp(end));
            ui.painter().rect_stroke(
                rect_selection,
                0,
                Stroke::new(2.0, Color32::from_rgb(70, 140, 240)),
                StrokeKind::Inside,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            if ui.input(|i| i.pointer.primary_released()) {
                let normalize = |p: egui::Pos2| {
                    let x = ((p.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                    let y = ((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                    match rotation % 4 {
                        1 => (y, 1.0 - x),
                        2 => (1.0 - x, 1.0 - y),
                        3 => (1.0 - y, x),
                        _ => (x, y),
                    }
                };
                let (a, b) = (normalize(rect_selection.min), normalize(rect_selection.max));
                if rect_selection.width() >= 4.0 && rect_selection.height() >= 4.0 {
                    self.capture_for_extension(
                        page,
                        Bounds {
                            left: a.0.min(b.0),
                            top: a.1.min(b.1),
                            right: a.0.max(b.0),
                            bottom: a.1.max(b.1),
                        },
                        ui.ctx(),
                    );
                } else {
                    self.extensions.drag = None;
                }
            }
        }
        true
    }
}
