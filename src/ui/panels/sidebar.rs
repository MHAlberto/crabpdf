//! Document thumbnails, outline and per-document favorites.
use crate::ui::{Action, ReaderApp, Sidebar, icons};
use crabpdf_application::favorites::FavoriteAction;
use crabpdf_core::document::Zoom;
use eframe::egui::{self, RichText};

impl ReaderApp {
    pub(in crate::ui) fn sidebar_panel(&mut self, ui: &mut egui::Ui) {
        if self.home {
            return;
        }
        let was_visible = if self.fullscreen {
            self.fullscreen_sidebar
        } else {
            self.preferences.sidebar_open
        };
        let mut visible = was_visible;
        egui::Panel::left("sidebar")
            .default_size(210.0)
            .size_range(165.0..=340.0)
            .drag_to_open(false)
            .show_collapsible(ui, &mut visible, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (name, side) in [
                        ("Páginas", Sidebar::Thumbnails),
                        ("Índice", Sidebar::Outline),
                        ("Favoritos", Sidebar::Favorites),
                    ] {
                        if ui
                            .add(egui::Button::new(name).selected(self.sidebar == side))
                            .clicked()
                        {
                            self.sidebar = side;
                        }
                    }
                });
                ui.separator();
                match self.sidebar {
                    Sidebar::Thumbnails => {
                        let Some(tab) = self.tabs.get(self.active) else {
                            ui.label("Abre un documento.");
                            return;
                        };
                        let Some(document) = &tab.reader.state.document else {
                            ui.label("Esperando documento…");
                            return;
                        };
                        let count = document.info.page_count;
                        let current = document.page;
                        egui::ScrollArea::vertical()
                            .id_salt("thumbnails")
                            .show_rows(ui, 190.0, count, |ui, range| {
                                for page in range {
                                    let Some(mut key) = self.state().render_key_for(
                                        page,
                                        140.0,
                                        190.0,
                                        ui.ctx().pixels_per_point(),
                                        ui.input(|i| i.max_texture_side),
                                    ) else {
                                        continue;
                                    };
                                    key.zoom = Zoom::FitWidth;
                                    key.viewport_width = 140;
                                    key.viewport_height = 0;
                                    let display = self.need_page(key);
                                    ui.vertical_centered(|ui| {
                                        if let Some(display) = display {
                                            let scale = (160.0 / display.size.y).min(1.0);
                                            let response = ui.add(
                                                egui::Image::new(&display.texture)
                                                    .fit_to_exact_size(display.size * scale)
                                                    .sense(egui::Sense::click()),
                                            );
                                            if response.clicked() {
                                                self.tabs[self.active].go_to(page, true);
                                            }
                                        } else {
                                            ui.add_sized(
                                                [140.0, 160.0],
                                                egui::Label::new("Preparando…"),
                                            );
                                        }
                                        if ui
                                            .add(
                                                egui::Button::new(format!("{}", page + 1))
                                                    .selected(current == page),
                                            )
                                            .clicked()
                                        {
                                            self.tabs[self.active].go_to(page, true);
                                        }
                                    });
                                }
                            });
                    }
                    Sidebar::Outline => {
                        use crate::ui::tab_view::OutlineView;
                        let Some(tab) = self
                            .tabs
                            .get_mut(self.active)
                            .filter(|tab| tab.reader.state.document.is_some())
                        else {
                            ui.label("Abre un documento para ver su índice.");
                            return;
                        };
                        if matches!(tab.view.outline, OutlineView::Unrequested)
                            && let Some(worker) = &self.worker
                        {
                            tab.view.outline = match worker.outline(tab.reader.id) {
                                Ok(()) => OutlineView::Loading,
                                Err(error) => OutlineView::Failed(error),
                            };
                        }
                        match &tab.view.outline {
                            OutlineView::Unrequested | OutlineView::Loading => {
                                ui.label("Preparando el índice…");
                            }
                            OutlineView::Failed(error) => {
                                ui.label(error.to_string());
                            }
                            OutlineView::Ready(outline) if outline.is_empty() => {
                                ui.label("El documento no contiene un índice incorporado.");
                            }
                            OutlineView::Ready(outline) => {
                                let mut destination = None;
                                egui::ScrollArea::vertical().id_salt("outline").show_rows(
                                    ui,
                                    ui.spacing().interact_size.y,
                                    outline.len(),
                                    |ui, range| {
                                        for index in range {
                                            let item = &outline[index];
                                            ui.push_id(index, |ui| {
                                                ui.horizontal(|ui| {
                                                    ui.add_space((item.depth.min(12) * 10) as f32);
                                                    if ui
                                                        .add_enabled(
                                                            item.page.is_some(),
                                                            egui::Button::new(&item.title)
                                                                .frame(false)
                                                                .truncate(),
                                                        )
                                                        .on_hover_text(&item.title)
                                                        .clicked()
                                                    {
                                                        destination = item.page;
                                                    }
                                                });
                                            });
                                        }
                                    },
                                );
                                if let Some(page) = destination {
                                    tab.go_to(page, true);
                                }
                            }
                        }
                    }
                    Sidebar::Favorites => {
                        let Some(tab) = self
                            .tabs
                            .get(self.active)
                            .filter(|t| t.reader.state.document.is_some())
                        else {
                            ui.label("Abre un documento para ver sus favoritos.");
                            return;
                        };
                        let path = tab.reader.path.clone();
                        let current = tab.reader.state.document.as_ref().map_or(0, |d| d.page);
                        let name = path.file_name().unwrap_or_default().to_string_lossy();
                        ui.label(RichText::new(name.as_ref()).strong())
                            .on_hover_text(path.display().to_string());
                        let favorites = self.current_favorites();
                        let marked = favorites.iter().any(|f| f.page == current);
                        let toggle = ui
                            .add(
                                egui::Button::new(if marked {
                                    "Quitar esta página"
                                } else {
                                    "Guardar esta página"
                                })
                                .selected(marked),
                            )
                            .on_hover_text("Favorito del documento actual · Ctrl/⌘+B");
                        self.favorite_toggle = Some(toggle.rect);
                        if toggle.clicked() {
                            self.execute(Action::ToggleFavorite, ui.ctx());
                        }
                        ui.label(
                            RichText::new(format!("{} páginas guardadas", favorites.len())).small(),
                        );
                        ui.separator();
                        if favorites.is_empty() {
                            ui.label("Todavía no hay favoritos en este documento.");
                        }
                        let mut remove = None;
                        egui::ScrollArea::vertical()
                            .id_salt(("book-favorites", &path))
                            .show(ui, |ui| {
                                for favorite in favorites {
                                    ui.push_id(favorite.page, |ui| {
                                        ui.horizontal(|ui| {
                                            let default_old =
                                                format!("{name} · {}", favorite.page + 1);
                                            let label = if favorite.label == default_old {
                                                format!("Página {}", favorite.page + 1)
                                            } else {
                                                favorite.label.clone()
                                            };
                                            let width = (ui.available_width()
                                                - 24.0
                                                - ui.spacing().item_spacing.x)
                                                .max(40.0);
                                            let response = ui
                                                .add_sized(
                                                    [width, 28.0],
                                                    egui::Button::new(label)
                                                        .truncate()
                                                        .selected(favorite.page == current),
                                                )
                                                .on_hover_text(format!(
                                                    "Ir a la página {}",
                                                    favorite.page + 1
                                                ));
                                            self.favorite_buttons
                                                .push((favorite.page, response.rect));
                                            if response.clicked() {
                                                self.favorite_action(FavoriteAction::Navigate(
                                                    favorite.page,
                                                ));
                                            }
                                            if icons::close_button(
                                                ui,
                                                egui::vec2(24.0, 24.0),
                                                false,
                                                "Eliminar favorito",
                                            )
                                            .clicked()
                                            {
                                                remove = Some(favorite.page);
                                            }
                                        });
                                    });
                                }
                            });
                        if let Some(page) = remove {
                            self.favorite_action(FavoriteAction::Remove(page));
                        }
                    }
                }
            });
        if visible != was_visible {
            if self.fullscreen {
                self.fullscreen_sidebar = visible;
            } else {
                self.preferences.sidebar_open = visible;
                self.schedule_save();
            }
        }
    }
}
