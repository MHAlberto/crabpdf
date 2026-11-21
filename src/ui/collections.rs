//! Named snapshots of open reading tabs.
use super::*;
use crabpdf_application::profile::ReadingCollection;

#[derive(Default)]
pub(super) struct Collections {
    pub open: bool,
    name: String,
    removing: Option<String>,
}

impl ReaderApp {
    pub(super) fn show_collections(&mut self, ctx: &egui::Context) {
        if !self.collections.open {
            return;
        }
        let mut open = true;
        let mut save = false;
        let mut reopen = None;
        let mut delete = None;
        let collections = &mut self.collections;
        let available = self
            .tabs
            .iter()
            .any(|tab| tab.reader.state.document.is_some() || tab.reader.restore.is_some());
        egui::Window::new("Colecciones de lectura")
            .collapsible(false)
            .default_width(440.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let name = ui.add_enabled(
                        available,
                        egui::TextEdit::singleline(&mut collections.name)
                            .char_limit(80)
                            .hint_text("Nombre de la colección"),
                    );
                    let allowed = available
                        && !collections.name.trim().is_empty()
                        && self.preferences.collections.len() < 12;
                    save = ui
                        .add_enabled(allowed, egui::Button::new("Guardar pestañas abiertas"))
                        .clicked()
                        || (allowed
                            && name.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                });
                ui.separator();
                if self.preferences.collections.is_empty() {
                    ui.label("Guarda las pestañas abiertas para retomar una lectura en grupo.");
                }
                egui::ScrollArea::vertical()
                    .max_height(350.0)
                    .show(ui, |ui| {
                        for collection in &self.preferences.collections {
                            ui.push_id(&collection.id, |ui| {
                                ui.strong(&collection.name);
                                ui.small(format!("{} pestañas guardadas", collection.tabs.len()));
                                ui.horizontal_wrapped(|ui| {
                                    if ui.button("Abrir colección").clicked() {
                                        reopen = Some(collection.clone());
                                    }
                                    if ui.button("Eliminar…").clicked() {
                                        collections.removing = Some(collection.id.clone());
                                    }
                                });
                                if collections.removing.as_ref() == Some(&collection.id) {
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Eliminar colección").clicked() {
                                            delete = Some(collection.id.clone());
                                        }
                                        if ui.button("Cancelar").clicked() {
                                            collections.removing = None;
                                        }
                                    });
                                }
                                ui.separator();
                            });
                        }
                    });
            });
        self.collections.open = open;
        if save {
            self.save_collection();
        } else if let Some(collection) = reopen {
            self.open_collection(collection);
        } else if let Some(id) = delete {
            self.preferences.collections.retain(|item| item.id != id);
            self.collections.removing = None;
            self.save_preferences();
        }
    }

    fn save_collection(&mut self) {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .filter_map(|tab| {
                tab.stored().or_else(|| {
                    tab.reader.restore.clone().map(|mut saved| {
                        saved.view_id = Some(tab.view_id.clone());
                        saved
                    })
                })
            })
            .collect();
        if tabs.is_empty() || self.preferences.collections.len() >= 12 {
            return;
        }
        let id = match getrandom::u64() {
            Ok(id) => format!("{id:016x}"),
            Err(error) => {
                self.notice = Some(error.to_string());
                return;
            }
        };
        let active_view = self.tabs.get(self.active).map(|tab| tab.view_id.clone());
        self.preferences.collections.push(ReadingCollection {
            id,
            name: self.collections.name.trim().to_owned(),
            active_view,
            tabs,
        });
        self.collections.name.clear();
        self.save_preferences();
        self.notice = Some("Colección guardada.".into());
    }

    fn open_collection(&mut self, collection: ReadingCollection) {
        if self.worker.is_none() || collection.tabs.is_empty() {
            return;
        }
        let missing = collection
            .tabs
            .iter()
            .filter(|saved| {
                !self.tabs.iter().any(|tab| {
                    saved.view_id.as_ref() == Some(&tab.view_id) && tab.reader.path == saved.path
                })
            })
            .count();
        if self.tabs.len() + missing > 64 {
            self.notice = Some("Cierra algunas pestañas para abrir esta colección.".into());
            return;
        }
        let mut first = None;
        let mut active = None;
        for saved in collection.tabs {
            let chosen = saved.view_id.is_some() && saved.view_id == collection.active_view;
            let index = if let Some(index) = self.tabs.iter().position(|tab| {
                saved.view_id.as_ref() == Some(&tab.view_id) && tab.reader.path == saved.path
            }) {
                index
            } else {
                self.next_id += 1;
                let mut tab = Tab::new(self.next_id, saved.path.clone(), Some(saved));
                tab.deferred_open = true;
                self.tabs.push(tab);
                self.tabs.len() - 1
            };
            first.get_or_insert(index);
            if chosen {
                active = Some(index);
            }
        }
        if let Some(index) = active.or(first) {
            self.active = index;
            self.home = false;
            self.reveal_tab = true;
            self.open_deferred_tab();
            self.collections.open = false;
            self.schedule_save();
        }
    }
}
