//! Extension manager and user-selected page regions.
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
    InstalledCatalog(Inventory),
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

    fn show_catalog(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, repository: &Repository) {
        ui.strong("Instalar por código");
        ui.hyperlink_to("Abrir catálogo de extensiones", catalog::CATALOG_URL);
        ui.horizontal(|ui| {
            let code = ui.add_enabled(
                !self.busy(),
                egui::TextEdit::singleline(&mut self.code)
                    .hint_text("Código de 6 cifras")
                    .char_limit(6)
                    .desired_width(144.0),
            );
            let search = ui.add_enabled(
                !self.busy() && valid_code(self.code.trim()),
                egui::Button::new("Buscar"),
            );
            if !self.busy()
                && valid_code(self.code.trim())
                && (search.clicked()
                    || (code.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))))
            {
                let code = self.code.trim().to_owned();
                self.operation_label = "Consultando el catálogo…";
                self.catalog_preview = None;
                self.preview = None;
                self.manage(ctx, move || {
                    catalog::lookup(&code)
                        .map(|package| ManagementResult::Catalog(Box::new(package)))
                });
            }
        });

        for index in 0..self.updates.len() {
            let entry = &self.updates[index].entry;
            if ui
                .button(format!("Ver {} {}", entry.name, entry.version))
                .clicked()
            {
                self.catalog_preview = Some(Box::new(self.updates.remove(index)));
                break;
            }
        }
        if let Some(package) = &self.catalog_preview {
            let entry = &package.entry;
            ui.strong(format!("{} {}", entry.name, entry.version));
            ui.label(&entry.description);
            ui.label(format!(
                "{} · {:.1} MiB",
                package.asset.target,
                package.asset.bytes as f64 / (1024.0 * 1024.0)
            ));
            ui.label("La extensión se ejecuta con los permisos de tu usuario.");
            let installed = self.inventory.packages.iter().any(|existing| {
                existing.manifest.id == entry.id && existing.manifest.version == entry.version
            });
            ui.horizontal(|ui| {
                if installed {
                    ui.label("Esta versión ya está instalada.");
                }
                if ui
                    .add_enabled(
                        !self.busy() && !installed,
                        egui::Button::new("Descargar e instalar"),
                    )
                    .clicked()
                {
                    self.operation_label = "Descargando y verificando el paquete…";
                    let package = self.catalog_preview.take().expect("a preview is displayed");
                    let repository = repository.clone();
                    self.manage(ctx, move || {
                        catalog::install(&repository, &package)?;
                        repository
                            .inventory()
                            .map(ManagementResult::InstalledCatalog)
                    });
                }
                if ui.button("Descartar").clicked() {
                    self.catalog_preview = None;
                }
            });
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, data: &std::path::Path, can_select: bool) -> bool {
        let mut arm = false;
        let repository = Repository::new(data);
        self.show_tool(ctx, &repository, data, can_select, &mut arm);
        self.panels.notifications(ctx);
        if !self.open {
            return arm;
        }
        let mut open = true;
        let mut open_requested = false;
        egui::Window::new("Gestor de extensiones")
            .default_width(570.0)
            .default_height(450.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.show_catalog_tab, false, "Instaladas");
                    ui.selectable_value(&mut self.show_catalog_tab, true, "Catálogo");
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(560.0)
                    .show(ui, |ui| {
                        if self.show_catalog_tab {
                            self.show_catalog(ui, ctx, &repository);
                            self.show_manager_status(ui, ctx);
                            return;
                        }
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
                                .add_enabled(
                                    !self.busy(),
                                    egui::Button::new("Buscar actualizaciones"),
                                )
                                .clicked()
                            {
                                self.show_catalog_tab = true;
                                let installed = self
                                    .inventory.packages
                                    .iter()
                                    .map(|p| (p.manifest.id.clone(), p.manifest.version.clone()))
                                    .chain(self.inventory.issues.iter().filter_map(|issue| {
                                        Some((issue.id.clone(), issue.version.clone()?))
                                    }))
                                    .collect();
                                self.operation_label = "Buscando actualizaciones…";
                                self.manage(ctx, move || {
                                    catalog::updates(installed).map(ManagementResult::Updates)
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
                                        repository.inventory().map(ManagementResult::Packages)
                                    });
                                }
                                if ui.button("Descartar").clicked() {
                                    self.preview = None;
                                }
                            });
                        }
                        if self.inventory.over_limit {
                            ui.label("Se alcanzó el límite del inventario. Desinstala versiones para ver las restantes.");
                        }
                        for index in 0..self.inventory.issues.len() {
                            let issue = &self.inventory.issues[index];
                            let id = issue.id.clone();
                            let version = issue.version.clone();
                            ui.group(|ui| {
                                ui.strong(&id);
                                ui.label(&self.inventory.issues[index].detail);
                                if let Some(version) = version
                                    && ui.add_enabled(!self.busy(), egui::Button::new("Desinstalar copia dañada")).clicked()
                                {
                                    let repository = repository.clone();
                                    self.manage(ctx, move || {
                                        repository.remove_version(&id, &version)?;
                                        repository.inventory().map(ManagementResult::Packages)
                                    });
                                }
                            });
                        }
                        if self.inventory.packages.is_empty() {
                            ui.label("Instala un paquete para añadir herramientas al lector.");
                        }
                        for package in self.inventory.packages.clone() {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.strong(&package.manifest.name);
                                ui.label(format!("v{}", package.manifest.version));
                                ui.small(if package.enabled {
                                    "Activa"
                                } else {
                                    "Desactivada"
                                });
                            });
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
                                        repository.inventory().map(ManagementResult::Packages)
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
                                        repository.inventory().map(ManagementResult::Packages)
                                    });
                                }
                            });
                            if package.enabled {
                                if ui.button("Abrir extensión").clicked() {
                                    self.panels.open(&package);
                                    self.open = false;
                                    open_requested = true;
                                }
                                for command in &package.manifest.commands {
                                    let ready = !self.busy()
                                        && (command.input == InputKind::None
                                            || self.capture.as_ref().is_some_and(|capture| {
                                                command.input == InputKind::Region
                                                    || !capture.text.trim().is_empty()
                                            }));
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
                            self.capture_error = None;
                            self.selected = None;
                            self.open = false;
                            arm = true;
                        }
                        ui.label("También puedes usar Texto / Enviar selección a extensión.");
                        self.show_manager_status(ui, ctx);
                    });
            });
        self.open = open && !arm && !open_requested;
        arm
    }
    fn show_tool(
        &mut self,
        ctx: &egui::Context,
        repository: &Repository,
        data: &std::path::Path,
        can_select: bool,
        arm: &mut bool,
    ) {
        if self.selecting {
            return;
        }
        let Some(package) = self
            .inventory
            .packages
            .iter()
            .find(|p| p.enabled && self.panels.active.as_ref() == Some(&p.manifest.id))
            .cloned()
        else {
            return;
        };
        let mut open = true;
        egui::Window::new(&package.manifest.name)
            .id(egui::Id::new(("extension-tool", &package.manifest.id)))
            .default_width(if package.manifest.timer.is_some() {
                240.0
            } else {
                400.0
            })
            .default_pos(ctx.content_rect().right_top() + egui::vec2(-445.0, 95.0))
            .open(&mut open)
            .show(ctx, |ui| {
                if package.manifest.timer.is_some() {
                    self.panels.timer(ui, &package.manifest.id);
                } else {
                    ui.horizontal_wrapped(|ui| {
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
                            self.capture_error = None;
                            self.selected = None;
                            *arm = true;
                        }
                        for command in &package.manifest.commands {
                            let ready = !self.busy()
                                && (command.input == InputKind::None
                                    || self.capture.as_ref().is_some_and(|capture| {
                                        command.input == InputKind::Region
                                            || !capture.text.trim().is_empty()
                                    }));
                            if ui
                                .add_enabled(ready, egui::Button::new(&command.title))
                                .clicked()
                            {
                                self.run(ctx, repository.clone(), package.clone(), command.clone());
                            }
                        }
                    });
                    self.show_capture_status(ui);
                    self.show_run_status(ui, ctx, &package_key(&package));
                }
                ui.separator();
                self.panels.shortcuts(ui, &package.manifest.id, data);
            });
        if !open {
            self.panels.close();
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

    fn show_manager_status(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.show_capture_status(ui);
        if self.operation.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if self.operation_label.is_empty() {
                    "Actualizando extensiones…"
                } else {
                    self.operation_label
                });
            });
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }
        if let Some(key) = self.latest.clone()
            && (self.results.contains_key(&key)
                || self.running.as_ref().is_some_and(|run| run.owner == key))
        {
            if let Some(package) = self
                .inventory
                .packages
                .iter()
                .find(|p| p.manifest.id == key.0 && p.manifest.version == key.1)
            {
                ui.strong(&package.manifest.name);
            }
            self.show_run_status(ui, ctx, &key);
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
        match &mut entry.result {
            Err(error) => {
                ui.label(error.as_str());
            }
            Ok(output) => {
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
                ui.add(
                    egui::TextEdit::multiline(&mut output.text)
                        .code_editor()
                        .desired_rows(5)
                        .desired_width(f32::INFINITY),
                );
            }
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
                image: Some((capture.png.clone(), capture.width, capture.height)),
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
                match result {
                    Ok(ManagementResult::Packages(packages)) => {
                        self.extensions.inventory = packages;
                        self.extensions.retain_results();
                        self.extensions.message = None;
                    }
                    Ok(ManagementResult::InstalledCatalog(packages)) => {
                        self.extensions.inventory = packages;
                        self.extensions.retain_results();
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
