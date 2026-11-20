//! Extension manager and user-selected page regions.
mod manager;
mod panels;

use super::*;
use crate::desktop::extensions::{
    InstalledPackage, Inventory, Invocation, Repository, RunningExtension,
    catalog::{self, CatalogPackage},
};
use crabpdf_core::document::{Bounds, SelectionCapture};
use crabpdf_extension_api::catalog::valid_code;
use crabpdf_extension_api::{ExtensionCommand, ExtensionOutput, InputKind, Region};
use eframe::egui::{Color32, Rect, Stroke, StrokeKind};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};

enum ManagementResult {
    Packages(Inventory),
    Updates(Vec<CatalogPackage>),
    Installed(Inventory, PackageKey),
    Catalog(Box<CatalogPackage>),
    Preview(PathBuf, Box<crabpdf_extension_api::Manifest>),
}
type PackageKey = (String, String);
type DocumentRevision = (DocumentId, u64);

struct ExtensionRun {
    owner: PackageKey,
    document: Option<DocumentRevision>,
    process: RunningExtension,
    cancelling: bool,
    discarded: bool,
}

struct ExtensionResult {
    document: Option<DocumentRevision>,
    result: Result<ExtensionOutput, String>,
    reveal: bool,
}

fn package_key(package: &InstalledPackage) -> PackageKey {
    (
        package.manifest.id.clone(),
        package.manifest.version.clone(),
    )
}

#[derive(Default)]
pub(super) struct Extensions {
    pub open: bool,
    panels: panels::Panels,
    loaded: bool,
    show_catalog_tab: bool,
    focused: Option<PackageKey>,
    pending_remove: Option<PackageKey>,
    operation_label: &'static str,
    code: String,
    updates: Vec<CatalogPackage>,
    catalog_preview: Option<Box<CatalogPackage>>,
    inventory: Inventory,
    operation: Option<mpsc::Receiver<Result<ManagementResult, String>>>,
    picker: Option<crate::desktop::dialogs::PendingDialog>,
    preview: Option<(PathBuf, crabpdf_extension_api::Manifest)>,
    pub selecting: bool,
    drag: Option<(DocumentId, usize, egui::Pos2)>,
    selected: Option<(DocumentRevision, usize, Bounds)>,
    capture: Option<SelectionCapture>,
    pending: Option<u64>,
    capture_error: Option<String>,
    serial: u64,
    running: Option<ExtensionRun>,
    results: BTreeMap<PackageKey, ExtensionResult>,
    latest: Option<PackageKey>,
    message: Option<String>,
}
impl Extensions {
    pub fn result_text(&self) -> Option<&str> {
        self.results
            .get(self.latest.as_ref()?)?
            .result
            .as_ref()
            .ok()
            .map(|output| output.text.as_str())
    }
    fn busy(&self) -> bool {
        self.operation.is_some()
            || self.picker.is_some()
            || self.running.is_some()
            || self.pending.is_some()
    }
    fn manage(
        &mut self,
        ctx: &egui::Context,
        work: impl FnOnce() -> std::io::Result<ManagementResult> + Send + 'static,
    ) {
        self.message = None;
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
    pub(super) fn menu_entries(&mut self, ui: &mut egui::Ui) {
        if self
            .inventory
            .packages
            .iter()
            .any(|package| package.enabled)
        {
            ui.separator();
        }
        for package in self
            .inventory
            .packages
            .iter()
            .filter(|package| package.enabled)
        {
            ui.menu_button(&package.manifest.name, |ui| {
                if ui.button("Abrir").clicked() {
                    self.panels.open(package);
                    self.open = false;
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.panels.active.as_deref() == Some(&package.manifest.id),
                        egui::Button::new("Cerrar"),
                    )
                    .clicked()
                {
                    self.panels.close();
                    ui.close();
                }
            });
        }
    }

    fn retain_results(&mut self) {
        self.results.retain(|key, _| {
            self.inventory
                .packages
                .iter()
                .any(|p| p.enabled && p.manifest.id == key.0 && p.manifest.version == key.1)
        });
    }

    fn show_capture_status(&self, ui: &mut egui::Ui) {
        if self.pending.is_some() {
            ui.label("Preparando el recorte…");
        }
        if let Some(error) = &self.capture_error {
            ui.label(error);
        }
    }

    fn show_run_status(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, key: &PackageKey) {
        if let Some(run) = &mut self.running
            && &run.owner == key
            && !run.discarded
        {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if run.cancelling {
                    "Cancelando…"
                } else {
                    "Ejecutando extensión…"
                });
                if ui
                    .add_enabled(!run.cancelling, egui::Button::new("Cancelar"))
                    .clicked()
                {
                    run.process.cancel();
                    run.cancelling = true;
                }
            });
        }
        let Some(entry) = self.results.get_mut(key) else {
            return;
        };
        let response = match &mut entry.result {
            Err(error) => ui.label(error.as_str()),
            Ok(output) => {
                ui.vertical(|ui| {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.strong(match output.kind {
                            crabpdf_extension_api::OutputKind::Text => "Resultado",
                            crabpdf_extension_api::OutputKind::Latex => "LaTeX",
                        });
                        if ui.button("Copiar resultado").clicked() {
                            ctx.copy_text(output.text.clone());
                        }
                    });
                    egui::ScrollArea::vertical()
                        .id_salt("extension-output")
                        .max_height(144.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut output.text)
                                    .code_editor()
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                })
                .response
            }
        };
        if std::mem::take(&mut entry.reveal) {
            response.scroll_to_me(None);
        }
    }

    fn run(
        &mut self,
        ctx: &egui::Context,
        repository: Repository,
        package: InstalledPackage,
        command: ExtensionCommand,
    ) {
        let document = if command.input == InputKind::None {
            None
        } else {
            self.selected.map(|(identity, _, _)| identity)
        };
        let input = if command.input == InputKind::None {
            Invocation {
                command,
                page: 1,
                region: Region {
                    left: 0.0,
                    top: 0.0,
                    right: 1.0,
                    bottom: 1.0,
                },
                text: String::new(),
                image: None,
            }
        } else {
            let (Some((_, page, bounds)), Some(capture)) = (self.selected, self.capture.as_ref())
            else {
                return;
            };
            let image = (command.input == InputKind::Region)
                .then(|| (capture.png.clone(), capture.width, capture.height));
            Invocation {
                command,
                page: page + 1,
                region: Region {
                    left: bounds.left,
                    top: bounds.top,
                    right: bounds.right,
                    bottom: bounds.bottom,
                },
                text: capture.text.clone(),
                image,
            }
        };
        let owner = package_key(&package);
        self.latest = Some(owner.clone());
        self.results.remove(&owner);
        let repaint = ctx.clone();
        match RunningExtension::spawn(repository, package, input, move || {
            repaint.request_repaint()
        }) {
            Ok(process) => {
                self.running = Some(ExtensionRun {
                    owner,
                    document,
                    process,
                    cancelling: false,
                    discarded: false,
                });
            }
            Err(error) => {
                self.results.insert(
                    owner,
                    ExtensionResult {
                        document,
                        result: Err(error.to_string()),
                        reveal: true,
                    },
                );
            }
        }
    }
    fn invalidate_document(&mut self, identity: Option<DocumentRevision>) {
        if self
            .selected
            .is_some_and(|(selected, _, _)| Some(selected) != identity)
        {
            self.capture = None;
            self.pending = None;
            self.selected = None;
            self.capture_error = None;
        }
        if let Some(run) = &mut self.running
            && !run.discarded
            && run
                .document
                .is_some_and(|document| Some(document) != identity)
        {
            run.process.cancel();
            run.discarded = true;
        }
        self.results.retain(|_, entry| {
            entry
                .document
                .is_none_or(|document| Some(document) == identity)
        });
    }

    fn poll_run(&mut self) {
        if let Some(result) = self.running.as_ref().and_then(|run| run.process.poll()) {
            let run = self.running.take().expect("polled an active run");
            if !run.discarded {
                self.results.insert(
                    run.owner,
                    ExtensionResult {
                        document: run.document,
                        result,
                        reveal: true,
                    },
                );
            }
        }
    }

    pub(super) fn close_tool(&mut self) -> bool {
        self.panels.close()
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
        self.extensions.capture_error = None;
        self.extensions.selected = None;
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.view.selection_drag = None;
        }
    }
    pub(super) fn extensions_open(&mut self, ctx: &egui::Context) {
        self.extensions.open = true;
        self.extensions.loaded = true;
        if self.extensions.busy() {
            return;
        }
        let repository = Repository::new(&self.app_paths.data);
        self.extensions.manage(ctx, move || {
            repository.inventory().map(ManagementResult::Packages)
        });
    }
    pub(super) fn poll_extensions(&mut self, ctx: &egui::Context) {
        if !self.extensions.loaded {
            self.extensions.loaded = true;
            let repository = Repository::new(&self.app_paths.data);
            if repository.has_installed() {
                self.extensions.manage(ctx, move || {
                    repository.inventory().map(ManagementResult::Packages)
                });
            }
        }
        if self.extensions.inventory.packages.is_empty()
            && self.extensions.operation.is_none()
            && !self.extensions.open
            && !self.extensions.selecting
            && self.extensions.selected.is_none()
            && self.extensions.pending.is_none()
            && self.extensions.running.is_none()
            && self.extensions.picker.is_none()
        {
            return;
        }
        self.extensions.panels.poll(
            ctx,
            &self.app_paths.data,
            &self.extensions.inventory.packages,
        );
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
        self.extensions.invalidate_document(identity);
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
                self.extensions.operation_label = "";
                match result {
                    Ok(ManagementResult::Packages(packages)) => {
                        self.extensions.inventory = packages;
                        self.extensions.retain_results();
                        self.extensions.message = None;
                    }
                    Ok(ManagementResult::Installed(packages, key)) => {
                        self.extensions.inventory = packages;
                        self.extensions.retain_results();
                        self.extensions.focused = Some(key);
                        self.extensions.preview = None;
                        self.extensions.catalog_preview = None;
                        self.extensions.updates.retain(|p| {
                            self.extensions
                                .focused
                                .as_ref()
                                .is_none_or(|key| p.entry.id != key.0 || p.entry.version != key.1)
                        });
                        self.extensions.show_catalog_tab = false;
                        self.extensions.message =
                            Some("Extensión instalada. Pulsa Activar para usarla.".into());
                    }
                    Ok(ManagementResult::Updates(updates)) => {
                        if updates.is_empty() {
                            self.extensions.message = Some("No hay actualizaciones compatibles para las extensiones instaladas.".into());
                        }
                        self.extensions.updates = updates;
                    }
                    Ok(ManagementResult::Catalog(package)) => {
                        self.extensions.catalog_preview = Some(package);
                    }
                    Ok(ManagementResult::Preview(path, manifest)) => {
                        self.extensions.catalog_preview = None;
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
                self.extensions.operation_label = "Revisando paquete local…";
                self.extensions.manage(ctx, move || {
                    repository
                        .inspect(&path)
                        .map(|manifest| ManagementResult::Preview(path, Box::new(manifest)))
                });
            }
        }
        self.extensions.poll_run();
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
        self.extensions.serial = self.extensions.serial.wrapping_add(1);
        let token = self.extensions.serial;
        let identity = (tab.reader.id, tab.reader.state.content_revision);
        self.extensions.selected = Some((identity, page, bounds));
        self.extensions.capture_error = None;
        self.extensions.capture = None;
        self.notice = None;
        self.extensions.selecting = false;
        self.extensions.drag = None;
        self.extensions.open = self.extensions.panels.active.is_none();
        if let Some(worker) = &self.worker {
            match worker.capture(
                tab.reader.id,
                token,
                tab.reader.state.content_revision,
                page,
                bounds,
                tab.reader.state.rotation,
            ) {
                Ok(()) => {
                    self.extensions.pending = Some(token);
                }
                Err(e) => self.extensions.capture_error = Some(e.to_string()),
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
        if self.extensions.pending != Some(token)
            || self
                .extensions
                .selected
                .is_none_or(|((requested, _), _, _)| requested != id)
        {
            return;
        }
        self.extensions.pending = None;
        match result {
            Ok(capture) => self.extensions.capture = Some(capture),
            Err(error) => self.extensions.capture_error = Some(error.to_string()),
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
