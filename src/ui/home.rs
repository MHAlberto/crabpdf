use super::{Action, ReaderApp, icons, theme};
use eframe::egui::{self, Color32, Rect, RichText, Sense};
use std::path::Path;
use std::sync::Arc;

impl ReaderApp {
    pub(super) fn library_home(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(self.preferences.theme);
        let mut open = None;
        let mut forget = None;
        egui::Frame::NONE.inner_margin(16).show(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("library-home").show(ui, |ui| {
            ui.add_space(24.0);
            ui.horizontal(|ui| {
                ui.heading("Tu biblioteca");
                ui.add_space(14.0);
                if ui.button("Abrir documento…").clicked() { self.execute(Action::Open, ui.ctx()); }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.selectable_label(self.library_frequent, "Frecuentes").clicked() { self.library_frequent = true; }
                    if ui.selectable_label(!self.library_frequent, "Recientes").clicked() { self.library_frequent = false; }
                });
            });
            ui.label(RichText::new("Continúa donde dejaste la lectura. Inicio conserva tus documentos abiertos.").color(palette.muted));
            ui.add_space(20.0);
            let mut documents = self.preferences.recent.clone();
            if self.library_frequent {
                documents.sort_by_key(|saved| std::cmp::Reverse(self.preferences.library.iter().find(|entry| entry.path == saved.path).map_or(0, |entry| entry.opens)));
            }
            if documents.is_empty() {
                ui.add_space(40.0);
                ui.vertical_centered(|ui| {
                    ui.heading("Un espacio para leer.");
                    ui.label("Abre un PDF, libro, cómic o imagen, o arrástralo a esta ventana.");
                    ui.label(RichText::new("Ctrl / ⌘ + O").color(palette.muted));
                });
            }
            let columns = ((ui.available_width() + 16.0) / 210.0).floor().clamp(1.0, 6.0) as usize;
            let card_width = ((ui.available_width() - 16.0 * (columns - 1) as f32) / columns as f32).min(238.0);
            for row in documents.chunks(columns) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    for saved in row {
                        egui::Frame::new().fill(palette.surface).corner_radius(5).inner_margin(8).show(ui, |ui| {
                            ui.vertical(|ui| {
                            ui.set_width(card_width - 16.0);
                            let (rect, response) = ui.allocate_exact_size(egui::vec2(card_width - 16.0, 160.0), Sense::click());
                            let visible = ui.is_rect_visible(rect);
                            let texture = if visible { self.need_cover(&saved.path) } else { None };
                            if let Some(texture) = texture {
                                let image_size = texture.size_vec2();
                                let scale = (rect.width() / image_size.x).min(rect.height() / image_size.y);
                                let target = Rect::from_center_size(rect.center(), image_size * scale);
                                ui.painter().image(texture.id(), target, Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0,1.0)), Color32::WHITE);
                            } else {
                                icons::paint(ui, icons::Icon::Document, Rect::from_center_size(rect.center(), egui::vec2(38.0,48.0)), palette.muted);
                            }
                            let name = saved.path.file_name().unwrap_or_default().to_string_lossy();
                            let title = self.preferences.library.iter().find(|entry| entry.path == saved.path).map(|entry| entry.title.as_str()).filter(|title| !title.is_empty()).unwrap_or(&name);
                            let label = ui.add(egui::Label::new(title).truncate().sense(Sense::click()));
                            ui.add(egui::Label::new(RichText::new(name.as_ref()).small().color(palette.muted)).truncate());
                            if let Some(error) = self.cover_errors.get(&saved.path) {
                                ui.label(RichText::new("Portada no disponible").small().color(palette.muted)).on_hover_text(error);
                            } else {
                                ui.label(RichText::new(format!("Continuar en página {}", saved.page + 1)).small().color(palette.muted));
                            }
                            let response = response.union(label).on_hover_text(saved.path.display().to_string());
                            if response.clicked() { open = Some(saved.clone()); }
                            response.context_menu(|ui| {
                                if ui.button("Abrir documento").clicked() { open = Some(saved.clone()); ui.close(); }
                                if ui.button("Quitar del historial").on_hover_text("Conserva el archivo PDF en su carpeta.").clicked() { forget = Some(saved.path.clone()); ui.close(); }
                                if self.cover_errors.contains_key(&saved.path) && ui.button("Volver a intentar portada").clicked() { self.cover_errors.remove(&saved.path); ui.close(); }
                            });
                            });
                        });
                    }
                });
                ui.add_space(16.0);
            }
        });
        });
        if let Some(saved) = open {
            self.open_document(saved.path.clone(), Some(saved));
        }
        if let Some(path) = forget {
            self.forget_recent(&path);
        }
    }

    fn need_cover(&mut self, path: &Path) -> Option<Arc<egui::TextureHandle>> {
        let path = path.to_owned();
        if let Some(texture) = self.covers.get(&path) {
            return Some(texture);
        }
        if self.cover_errors.contains_key(&path) {
            return None;
        }
        if self.pending_covers.insert(path.clone())
            && let Some(worker) = &self.worker
        {
            let cache = Some(self.app_paths.data.join("covers"));
            if let Err(error) = worker.cover(path.clone(), cache) {
                self.pending_covers.remove(&path);
                self.cover_errors.insert(path, error.to_string());
            }
        }
        None
    }

    pub fn home_active(&self) -> bool {
        self.home
    }
    pub fn library_cover_ready(&self, path: &Path) -> bool {
        self.covers
            .contains(&path.canonicalize().unwrap_or_else(|_| path.to_owned()))
    }
    pub fn forget_recent(&mut self, path: &Path) {
        let normalized = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        let path = normalized.as_path();
        self.preferences.forget(path);
        self.covers.retain(|saved| saved != path);
        self.cover_errors.remove(path);
        self.save_preferences();
    }
    pub fn recent_count(&self) -> usize {
        self.preferences.recent.len()
    }
}
