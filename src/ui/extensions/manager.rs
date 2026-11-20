//! Native extension workspace. Package and process operations stay in the controller.
use super::*;

use crabpdf_extension_api::{Manifest, OutputKind};

fn palette(ui: &egui::Ui) -> theme::Palette {
    theme::palette(if ui.visuals().dark_mode {
        Theme::Dark
    } else {
        Theme::Light
    })
}

fn primary(ui: &mut egui::Ui, enabled: bool, label: &str) -> egui::Response {
    let colors = palette(ui);
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).strong().color(colors.accent))
            .fill(colors.accent.gamma_multiply(0.10))
            .stroke(Stroke::new(1.0, colors.accent)),
    )
}

fn muted(ui: &mut egui::Ui, label: impl Into<egui::RichText>) {
    ui.label(label.into().color(palette(ui).muted));
}

fn surface(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui)) {
    let colors = palette(ui);
    egui::Frame::new()
        .fill(colors.canvas)
        .stroke(Stroke::new(1.0, colors.border))
        .corner_radius(8)
        .inner_margin(14)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                contents(ui);
            });
        });
}

fn package_header(ui: &mut egui::Ui, name: &str, detail: &str) {
    ui.label(egui::RichText::new(name).size(21.0).strong());
    muted(ui, detail);
}

fn command_data(command: &ExtensionCommand) -> &'static str {
    match (command.input, command.output) {
        (InputKind::Region, OutputKind::Latex) => "Recorte del PDF · LaTeX",
        (InputKind::Region, OutputKind::Text) => "Recorte del PDF · Texto",
        (InputKind::Text, OutputKind::Latex) => "Texto seleccionado · LaTeX",
        (InputKind::Text, OutputKind::Text) => "Texto seleccionado · Texto",
        (InputKind::None, OutputKind::Latex) => "LaTeX",
        (InputKind::None, OutputKind::Text) => "Texto",
    }
}

impl Extensions {
    pub fn show(&mut self, ctx: &egui::Context, data: &std::path::Path, can_select: bool) -> bool {
        let repository = Repository::new(data);
        self.panels.notifications(ctx);
        if !self.open {
            return self.show_tool(ctx, &repository, data, can_select);
        }

        let mut open = true;
        let mut arm = false;
        let screen = ctx.content_rect();
        let viewport = screen.size();
        let width = (viewport.x - 48.0).clamp(250.0, 744.0);
        let height = (viewport.y - 120.0).clamp(260.0, 450.0);
        egui::Window::new("Extensiones")
            .id(egui::Id::new("extensions-workspace"))
            .collapsible(false)
            .default_pos(screen.center() - egui::vec2(width * 0.5, height * 0.5))
            .default_width(width)
            .max_width((viewport.x - 24.0).max(250.0))
            .default_height(height)
            .max_height((viewport.y - 68.0).max(210.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.show_catalog_tab, false, "Instaladas");
                    ui.selectable_value(&mut self.show_catalog_tab, true, "Catálogo");
                    if ui
                        .add_enabled(!self.busy(), egui::Button::new("Desde carpeta…"))
                        .on_hover_text("Revisar un paquete local")
                        .clicked()
                    {
                        self.preview = None;
                        self.picker =
                            Some(crate::desktop::dialogs::PendingDialog::extension_folder());
                    }
                });
                ui.separator();
                self.show_manager_status(ui);
                egui::ScrollArea::vertical()
                    .id_salt("extension-workspace-body")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.preview.is_some() {
                            self.show_local_preview(ui, ctx, &repository);
                        } else if self.show_catalog_tab {
                            self.show_catalog(ui, ctx, &repository);
                        } else {
                            arm = self.show_installed(ui, ctx, &repository, data, can_select);
                        }
                    });
            });
        self.open &= open && !arm;
        if arm {
            self.panels.close();
        }
        arm
    }

    fn show_installed(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        repository: &Repository,
        data: &std::path::Path,
        can_select: bool,
    ) -> bool {
        let active = self.inventory.packages.iter().filter(|p| p.enabled).count();
        ui.horizontal_wrapped(|ui| {
            ui.strong("Biblioteca");
            muted(
                ui,
                format!(
                    "{} instaladas · {active} activas",
                    self.inventory.packages.len()
                ),
            );
            if ui
                .add_enabled(!self.busy(), egui::Button::new("Buscar actualizaciones"))
                .clicked()
            {
                self.show_catalog_tab = true;
                self.catalog_preview = None;
                let installed =
                    self.inventory
                        .packages
                        .iter()
                        .map(package_key)
                        .chain(
                            self.inventory.issues.iter().filter_map(|issue| {
                                Some((issue.id.clone(), issue.version.clone()?))
                            }),
                        )
                        .collect();
                self.operation_label = "Buscando actualizaciones…";
                self.manage(ctx, move || {
                    catalog::updates(installed).map(ManagementResult::Updates)
                });
            }
        });
        ui.add_space(4.0);
        let mut arm = false;
        if self.inventory.packages.is_empty() {
            if self.operation.is_none() {
                surface(ui, |ui| {
                    package_header(
                        ui,
                        "Añade tu primera extensión",
                        "Instala un paquete por código o desde una carpeta.",
                    );
                    if primary(ui, true, "Buscar por código").clicked() {
                        self.show_catalog_tab = true;
                    }
                });
            }
        } else {
            if self.focused.as_ref().is_none_or(|key| {
                !self
                    .inventory
                    .packages
                    .iter()
                    .any(|p| p.manifest.id == key.0 && p.manifest.version == key.1)
            }) {
                self.focused = self
                    .inventory
                    .packages
                    .iter()
                    .find(|p| p.enabled)
                    .or_else(|| self.inventory.packages.first())
                    .map(package_key);
            }
            if ui.available_width() >= 590.0 {
                let list_width = 228.0;
                let detail_width = ui.available_width() - list_width - ui.spacing().item_spacing.x;
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(list_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_max_width(list_width);
                            self.package_list(ui);
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(detail_width, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_max_width(detail_width);
                            surface(ui, |ui| {
                                arm = self.show_package(ui, ctx, repository, data, can_select);
                            });
                        },
                    );
                });
            } else {
                self.package_list(ui);
                surface(ui, |ui| {
                    arm = self.show_package(ui, ctx, repository, data, can_select);
                });
            }
        }
        if self.inventory.over_limit {
            muted(
                ui,
                "El inventario alcanzó su límite. Desinstala versiones para ver las restantes.",
            );
        }
        for issue in &self.inventory.issues {
            ui.strong(&issue.id);
            muted(ui, issue.detail.as_str());
            if let Some(version) = &issue.version
                && ui
                    .add_enabled(!self.busy(), egui::Button::new("Revisar desinstalación"))
                    .clicked()
            {
                self.pending_remove = Some((issue.id.clone(), version.clone()));
            }
            if let Some(version) = &issue.version
                && self.pending_remove.as_ref() == Some(&(issue.id.clone(), version.clone()))
            {
                let key = (issue.id.clone(), version.clone());
                let id = issue.id.clone();
                self.confirm_removal(ui, ctx, repository, &key, &id);
                break;
            }
        }
        arm
    }

    fn package_list(&mut self, ui: &mut egui::Ui) {
        for package in &self.inventory.packages {
            let key = package_key(package);
            let status = if package.enabled {
                "Activa"
            } else {
                "Inactiva"
            };
            let label = format!("{}\n{} · {}", package.manifest.name, key.1, status);
            let selected = self.focused.as_ref() == Some(&key);
            if ui
                .add_sized(
                    [ui.available_width(), 48.0],
                    egui::Button::selectable(selected, label).wrap(),
                )
                .clicked()
            {
                self.focused = Some(key);
                self.pending_remove = None;
            }
        }
    }

    fn show_package(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        repository: &Repository,
        data: &std::path::Path,
        can_select: bool,
    ) -> bool {
        let Some(key) = self.focused.clone() else {
            return false;
        };
        let Some(package) = self
            .inventory
            .packages
            .iter()
            .find(|p| p.manifest.id == key.0 && p.manifest.version == key.1)
            .cloned()
        else {
            return false;
        };
        let manifest = &package.manifest;
        let active_version = self
            .inventory
            .packages
            .iter()
            .find(|p| p.enabled && p.manifest.id == key.0);
        let switch_version = !package.enabled && active_version.is_some();
        let status = if package.enabled {
            "Activa"
        } else {
            "Inactiva"
        };
        package_header(ui, &manifest.name, &format!("Versión {} · {status}", key.1));
        ui.label(&manifest.description);
        if let Some(active) = active_version.filter(|_| switch_version) {
            muted(
                ui,
                format!("Versión activa actualmente · {}", active.manifest.version),
            );
        }

        ui.horizontal_wrapped(|ui| {
            let busy = self.busy();
            if package.enabled && primary(ui, !busy, "Abrir herramienta").clicked() {
                self.panels.open(&package);
                self.open = false;
            }
            let action = if package.enabled {
                "Desactivar"
            } else if switch_version {
                "Activar esta versión"
            } else {
                "Activar"
            };
            let toggle = if package.enabled {
                ui.add_enabled(!busy, egui::Button::new(action))
            } else {
                primary(ui, !busy, action)
            };
            if toggle.clicked() {
                let repository = repository.clone();
                let package = package.clone();
                self.operation_label = if package.enabled {
                    "Desactivando…"
                } else {
                    "Activando…"
                };
                self.manage(ctx, move || {
                    repository.enable(&package, !package.enabled)?;
                    repository.inventory().map(ManagementResult::Packages)
                });
            }
        });

        if package.enabled {
            if manifest.timer.is_none() && !manifest.commands.is_empty() {
                ui.separator();
                if self.show_commands(ui, ctx, repository, &package, can_select) {
                    return true;
                }
            }
            self.show_run_status(ui, ctx, &key);
            ui.separator();
            self.panels.shortcuts(ui, &key.0, data);
        }
        ui.collapsing("Detalles del paquete", |ui| {
            muted(ui, &key.0);
            muted(ui, &manifest.target);
            for command in &manifest.commands {
                ui.label(format!("{} · {}", command.title, command_data(command)));
            }
            if let Some(timer) = manifest.timer {
                ui.label(format!(
                    "{} min de enfoque · {} min de descanso",
                    timer.focus_minutes, timer.break_minutes
                ));
            }
            if ui
                .add_enabled(!self.busy(), egui::Button::new("Desinstalar…"))
                .clicked()
            {
                self.pending_remove = Some(key.clone());
            }
            if self.pending_remove.as_ref() == Some(&key) {
                self.confirm_removal(ui, ctx, repository, &key, &manifest.name);
            }
        });
        false
    }

    fn confirm_removal(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        repository: &Repository,
        key: &PackageKey,
        name: &str,
    ) {
        ui.label(format!("¿Desinstalar {name} {}?", key.1));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy(), egui::Button::new("Confirmar desinstalación"))
                .clicked()
            {
                let key = key.clone();
                let repository = repository.clone();
                self.pending_remove = None;
                self.operation_label = "Desinstalando paquete…";
                self.manage(ctx, move || {
                    repository.remove_version(&key.0, &key.1)?;
                    repository.inventory().map(ManagementResult::Packages)
                });
            }
            if ui.button("Conservar paquete").clicked() {
                self.pending_remove = None;
            }
        });
    }

    fn show_catalog(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, repository: &Repository) {
        package_header(
            ui,
            "Instalar por código",
            "Introduce el código de seis cifras de la extensión.",
        );
        ui.horizontal_wrapped(|ui| {
            let code = ui
                .add_enabled(
                    !self.busy(),
                    egui::TextEdit::singleline(&mut self.code)
                        .hint_text("Código de 6 cifras")
                        .char_limit(6)
                        .desired_width(152.0),
                )
                .on_hover_text("Código de la extensión");
            let valid = !self.busy() && valid_code(self.code.trim());
            if primary(ui, valid, "Consultar").clicked()
                || (valid && code.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                let code = self.code.trim().to_owned();
                self.operation_label = "Consultando el catálogo…";
                self.catalog_preview = None;
                self.manage(ctx, move || {
                    catalog::lookup(&code)
                        .map(|package| ManagementResult::Catalog(Box::new(package)))
                });
            }
            ui.hyperlink_to("Ver catálogo en la web", catalog::CATALOG_URL);
        });

        if !self.updates.is_empty() {
            ui.separator();
            ui.strong("Actualizaciones compatibles");
            for package in &self.updates {
                let entry = &package.entry;
                if ui
                    .add_enabled(
                        !self.busy(),
                        egui::Button::new(format!("{} · {}", entry.name, entry.version)),
                    )
                    .clicked()
                {
                    self.catalog_preview = Some(Box::new(package.clone()));
                }
            }
        }

        let Some(package) = self.catalog_preview.as_ref() else {
            return;
        };
        let entry = &package.entry;
        let key = (entry.id.clone(), entry.version.clone());
        let installed = self
            .inventory
            .packages
            .iter()
            .any(|p| p.manifest.id == key.0 && p.manifest.version == key.1);
        let size = package.asset.bytes as f64 / 1_048_576.0;
        let size_label = if size >= 1.0 {
            format!("{size:.1} MiB")
        } else {
            format!("{} KiB", package.asset.bytes.div_ceil(1024))
        };
        let mut install = false;
        let mut dismiss = false;
        let mut reveal = false;
        let can_install = !self.busy();
        let active = self
            .inventory
            .packages
            .iter()
            .find(|p| p.enabled && p.manifest.id == key.0);
        surface(ui, |ui| {
            package_header(
                ui,
                &entry.name,
                &format!("Versión {} · {} · {size_label}", entry.version, entry.code),
            );
            ui.label(&entry.description);
            if let Some(active) = active.filter(|_| !installed) {
                muted(
                    ui,
                    format!("Versión activa actualmente · {}", active.manifest.version),
                );
            }
            muted(
                ui,
                "Un paquete nativo se ejecuta con los permisos de tu usuario. Instala paquetes de confianza.",
            );
            ui.horizontal_wrapped(|ui| {
                if installed {
                    reveal = primary(ui, true, "Ver en instaladas").clicked();
                } else {
                    let label = if active.is_some() {
                        "Descargar actualización"
                    } else {
                        "Descargar e instalar"
                    };
                    install = primary(ui, can_install, label).clicked();
                }
                dismiss = ui.button("Cerrar ficha").clicked();
            });
        });
        if reveal {
            self.focused = Some(key);
            self.show_catalog_tab = false;
        } else if install {
            let package = package.clone();
            let repository = repository.clone();
            self.operation_label = "Descargando y verificando el paquete…";
            self.manage(ctx, move || {
                catalog::install(&repository, &package)?;
                repository
                    .inventory()
                    .map(|inventory| ManagementResult::Installed(inventory, key))
            });
        } else if dismiss {
            self.catalog_preview = None;
        }
    }

    fn show_local_preview(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        repository: &Repository,
    ) {
        let Some((path, manifest)) = self.preview.as_ref() else {
            return;
        };
        let key = (manifest.id.clone(), manifest.version.clone());
        let mut install = false;
        let mut dismiss = false;
        surface(ui, |ui| {
            package_header(
                ui,
                &manifest.name,
                &format!("Paquete local · Versión {}", manifest.version),
            );
            ui.label(&manifest.description);
            ui.label(path.display().to_string());
            for command in &manifest.commands {
                ui.label(format!("{} · {}", command.title, command_data(command)));
            }
            muted(
                ui,
                "Un paquete nativo se ejecuta con los permisos de tu usuario. Instala paquetes de confianza.",
            );
            ui.horizontal_wrapped(|ui| {
                install = primary(ui, !self.busy(), "Instalar paquete").clicked();
                dismiss = ui.button("Cancelar").clicked();
            });
        });
        if install {
            let path = path.clone();
            let manifest = manifest.clone();
            let repository = repository.clone();
            self.operation_label = "Instalando paquete…";
            self.manage(ctx, move || {
                repository.install_reviewed(&path, &manifest)?;
                repository
                    .inventory()
                    .map(|inventory| ManagementResult::Installed(inventory, key))
            });
        } else if dismiss {
            self.preview = None;
        }
    }

    fn show_manager_status(&mut self, ui: &mut egui::Ui) {
        if self.operation.is_some() || self.picker.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(if self.picker.is_some() {
                    "Elige la carpeta del paquete…"
                } else if self.operation_label.is_empty() {
                    "Cargando extensiones…"
                } else {
                    self.operation_label
                });
            });
        }
        if let Some(message) = &self.message {
            surface(ui, |ui| {
                ui.label(message);
            });
        }
    }

    fn show_commands(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        repository: &Repository,
        package: &InstalledPackage,
        can_select: bool,
    ) -> bool {
        let commands = &package.manifest.commands;
        if commands
            .iter()
            .any(|command| command.input != InputKind::None)
        {
            if let (Some((_, page, _)), Some(_)) = (self.selected, &self.capture) {
                ui.strong(format!("Selección lista · Página {}", page + 1));
            } else {
                muted(
                    ui,
                    if can_select {
                        "Recorta un área o usa Texto › Enviar selección a extensión."
                    } else {
                        "Abre un PDF que permita copiar para usar su contenido."
                    },
                );
            }
            let label = if self.capture.is_some() {
                "Cambiar selección…"
            } else {
                "Seleccionar área…"
            };
            if primary(ui, can_select && !self.busy(), label).clicked() {
                self.selecting = true;
                self.drag = None;
                self.capture = None;
                self.capture_error = None;
                self.selected = None;
                return true;
            }
            self.show_capture_status(ui);
            ui.add_space(4.0);
        }
        for command in commands {
            let ready = !self.busy()
                && match command.input {
                    InputKind::None => true,
                    InputKind::Region => self
                        .capture
                        .as_ref()
                        .is_some_and(|capture| !capture.png.is_empty()),
                    InputKind::Text => self
                        .capture
                        .as_ref()
                        .is_some_and(|capture| !capture.text.trim().is_empty()),
                };
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(ready, egui::Button::new(&command.title))
                    .clicked()
                {
                    self.run(ctx, repository.clone(), package.clone(), command.clone());
                }
                muted(ui, command_data(command));
            });
        }
        false
    }

    fn show_tool(
        &mut self,
        ctx: &egui::Context,
        repository: &Repository,
        data: &std::path::Path,
        can_select: bool,
    ) -> bool {
        if self.selecting {
            return false;
        }
        let Some(package) = self
            .inventory
            .packages
            .iter()
            .find(|p| p.enabled && self.panels.active.as_ref() == Some(&p.manifest.id))
            .cloned()
        else {
            return false;
        };
        let manifest: &Manifest = &package.manifest;
        let width = if manifest.timer.is_some() {
            238.0
        } else {
            386.0
        };
        let viewport = ctx.content_rect();
        let mut open = true;
        let mut arm = false;
        egui::Window::new(&manifest.name)
            .id(egui::Id::new(("extension-tool", &manifest.id)))
            .collapsible(false)
            .default_width(width)
            .default_pos(viewport.right_top() + egui::vec2(-width - 24.0, 95.0))
            .open(&mut open)
            .show(ctx, |ui| {
                if manifest.timer.is_some() {
                    surface(ui, |ui| self.panels.timer(ui, &manifest.id));
                } else {
                    surface(ui, |ui| {
                        arm = self.show_commands(ui, ctx, repository, &package, can_select);
                    });
                    self.show_run_status(ui, ctx, &package_key(&package));
                }
                self.panels.shortcuts(ui, &manifest.id, data);
            });
        if !open {
            self.panels.close();
        }
        arm
    }
}
