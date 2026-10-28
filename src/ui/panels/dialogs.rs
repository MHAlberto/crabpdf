//! Go-to, zoom, command palette and password dialogs.
use crate::desktop::command_meta::ActionMetadata;
use crate::ui::{Action, ReaderApp};
use crabpdf_core::document::Zoom;
use eframe::egui;

impl ReaderApp {
    pub(in crate::ui) fn dialogs(&mut self, ctx: &egui::Context) {
        self.password_dialog(ctx);
        if self.go_to_open {
            let mut submit = false;
            let mut cancel = false;
            let count = self
                .state()
                .document
                .as_ref()
                .map_or(1, |d| d.info.page_count);
            egui::Window::new("Ir a página")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut self.go_to_value).range(1..=count));
                        ui.label(format!("de {count}"));
                    });
                    ui.horizontal(|ui| {
                        submit = ui.button("Ir").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter));
                        cancel = ui.button("Cancelar").clicked();
                    });
                });
            if submit {
                if let Some(tab) = self.tabs.get_mut(self.active) {
                    tab.go_to(self.go_to_value.saturating_sub(1), true);
                }
                self.go_to_open = false;
                self.schedule_save();
            }
            if cancel {
                self.go_to_open = false;
            }
        }
        if self.zoom_open {
            let mut submit = false;
            let mut cancel = false;
            egui::Window::new("Zoom personalizado")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.zoom_value)
                            .range(8..=6400)
                            .suffix(" %"),
                    );
                    ui.horizontal(|ui| {
                        submit = ui.button("Aplicar").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter));
                        cancel = ui.button("Cancelar").clicked();
                    });
                });
            if submit {
                self.set_zoom(Zoom::Percent(self.zoom_value));
                self.zoom_open = false;
            }
            if cancel {
                self.zoom_open = false;
            }
        }
        if self.properties_open {
            let mut open = true;
            let properties = self
                .state()
                .details
                .as_ref()
                .map(|d| d.properties.clone())
                .unwrap_or_default();
            let path = self.tabs.get(self.active).map(|t| t.reader.path.clone());
            egui::Window::new("Propiedades del documento")
                .open(&mut open)
                .default_width(420.0)
                .show(ctx, |ui| {
                    if let Some(path) = &path {
                        ui.add(egui::Label::new(path.display().to_string()).wrap());
                        if let Ok(metadata) = path.metadata() {
                            ui.label(format!("Tamaño: {} bytes", metadata.len()));
                        }
                    }
                    ui.separator();
                    egui::Grid::new("document-properties")
                        .striped(true)
                        .show(ui, |ui| {
                            for (name, value) in properties {
                                ui.label(name);
                                ui.add(egui::Label::new(value).wrap());
                                ui.end_row();
                            }
                        });
                });
            self.properties_open = open;
        }
        if self.settings_open {
            let mut open = true;
            let mut changed = false;
            egui::Window::new("Preferencias")
                .open(&mut open)
                .show(ctx, |ui| {
                    changed |= ui
                        .checkbox(
                            &mut self.preferences.restore_session,
                            "Restaurar pestañas al iniciar",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(&mut self.preferences.sidebar_open, "Mostrar panel lateral")
                        .changed();
                    if !cfg!(target_os = "macos")
                        && ui
                            .checkbox(
                                &mut self.preferences.native_titlebar,
                                "Usar barra de título nativa",
                            )
                            .changed()
                    {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Decorations(
                                !self.fullscreen && self.preferences.native_titlebar,
                            ));
                        changed = true;
                    }
                    changed |= ui
                        .checkbox(&mut self.preferences.reduce_motion, "Reducir movimiento")
                        .changed();
                    if ui.button("Cambiar tema").clicked() {
                        self.toggle_theme(ctx);
                    }
                    ui.label("Las contraseñas no se guardan en las preferencias.");
                });
            self.settings_open = open;
            if changed {
                self.save_preferences();
            }
        }
        if self.palette_open {
            let mut open = true;
            let mut chosen = None;
            egui::Window::new("Comandos")
                .open(&mut open)
                .collapsible(false)
                .default_width(420.0)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 100.0])
                .show(ctx, |ui| {
                    let down = ui
                        .input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown));
                    let up =
                        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp));
                    let enter =
                        ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.palette_query)
                            .hint_text("Buscar comando"),
                    );
                    if self.palette_focus {
                        input.request_focus();
                        self.palette_focus = false;
                    }
                    if input.changed() {
                        self.palette_index = 0;
                        self.palette_reveal = true;
                    }
                    let query = fold(&self.palette_query);
                    let actions = Action::ALL
                        .iter()
                        .copied()
                        .filter(|a| fold(a.label()).contains(&query))
                        .collect::<Vec<_>>();
                    self.palette_index = self.palette_index.min(actions.len().saturating_sub(1));
                    if actions
                        .get(self.palette_index)
                        .is_some_and(|action| !self.enabled(*action))
                    {
                        self.palette_index = actions
                            .iter()
                            .position(|action| self.enabled(*action))
                            .unwrap_or(0);
                    }
                    if down || up {
                        let count = actions.len();
                        for offset in 1..=count {
                            let index = (self.palette_index
                                + if down { offset } else { count - offset })
                                % count;
                            if self.enabled(actions[index]) {
                                self.palette_index = index;
                                break;
                            }
                        }
                        self.palette_reveal = true;
                    }
                    egui::ScrollArea::vertical()
                        .max_height(340.0)
                        .show(ui, |ui| {
                            for (index, action) in actions.iter().enumerate() {
                                let response = ui.add_enabled(
                                    self.enabled(*action),
                                    egui::Button::new(action.label())
                                        .selected(index == self.palette_index)
                                        .shortcut_text(action.shortcut().unwrap_or(""))
                                        .min_size(egui::vec2(360.0, 26.0)),
                                );
                                if index == self.palette_index && self.palette_reveal {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    chosen = Some(*action);
                                }
                            }
                            if actions.is_empty() {
                                ui.label("No hay comandos que coincidan.");
                            }
                        });
                    self.palette_reveal = false;
                    if enter {
                        chosen = actions
                            .get(self.palette_index)
                            .copied()
                            .filter(|a| self.enabled(*a));
                    }
                });
            self.palette_open = open;
            if let Some(action) = chosen {
                self.palette_open = false;
                self.execute(action, ctx);
            }
        }
    }

    fn password_dialog(&mut self, ctx: &egui::Context) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        if !tab.reader.password_needed {
            return;
        }
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Documento protegido")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(
                    tab.reader
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                );
                ui.label("Introduce la contraseña para abrir este documento.");
                let response =
                    ui.add(egui::TextEdit::singleline(&mut tab.view.password).password(true));
                response.request_focus();
                ui.horizontal(|ui| {
                    submit = ui.button("Abrir").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    cancel = ui.button("Cancelar").clicked();
                });
            });
        if cancel {
            tab.reader.password_needed = false;
            tab.view.password.clear();
        }
        if submit {
            tab.reader.retry_open();
            let password = std::mem::take(&mut tab.view.password);
            if let Some(worker) = &self.worker
                && let Err(error) = worker.open_with_password(
                    tab.reader.id,
                    tab.reader.path.clone(),
                    Some(password),
                )
            {
                tab.reader.state.finish_open(tab.reader.id, Err(error));
            }
        }
    }
}
fn fold(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| match c {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' | 'ü' => 'u',
            other => other,
        })
        .collect()
}
