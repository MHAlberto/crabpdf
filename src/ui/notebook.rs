//! The document's notebook is loaded only when requested.
use super::*;
use crate::desktop::dialogs::PendingDialog;
use crabpdf_application::formats::Format;
use crabpdf_application::notebook::{Note, NoteAnchor, NoteKind};
use crabpdf_runtime::storage::notebook::{self, Change, MarkRecovery, NotebookKey};
use std::{io, path::Path, sync::mpsc, thread};

struct Book {
    id: DocumentId,
    key: NotebookKey,
    notes: Vec<Note>,
    marks: io::Result<MarkRecovery>,
    local_marks: Vec<crate::document::TextMarkup>,
}

struct Draft {
    document: DocumentId,
    original: Option<Note>,
    note: Note,
}

enum Outcome {
    Loaded(Book),
    Saved(Vec<Note>),
    Exported,
    Indexed,
    Restored(usize),
    MarksChanged,
}

struct Job {
    document: DocumentId,
    receive: mpsc::Receiver<io::Result<Outcome>>,
    write: Option<thread::JoinHandle<()>>,
}

#[derive(Default)]
pub(super) struct NotebookPanel {
    active: Option<DocumentId>,
    book: Option<Book>,
    draft: Option<Draft>,
    focus_draft: bool,
    job: Option<Job>,
    query: String,
    matches: Option<Vec<usize>>,
    notes_page: usize,
    message: Option<String>,
    confirm_delete: Option<u64>,
    confirm_mark_delete: Option<usize>,
    export: Option<(DocumentId, NotebookKey, String, Option<u64>, PendingDialog)>,
    load_attempted: bool,
    mark_request: Option<(DocumentId, PathBuf, Option<u64>)>,
    mark_indexed: Option<DocumentId>,
}

impl NotebookPanel {
    fn run(
        &mut self,
        ctx: &egui::Context,
        document: DocumentId,
        writing: bool,
        work: impl FnOnce() -> io::Result<Outcome> + Send + 'static,
    ) {
        self.message = None;
        let (send, receive) = mpsc::channel();
        let repaint = ctx.clone();
        match thread::Builder::new()
            .name("crabpdf-notebook".into())
            .spawn(move || {
                let _ = send.send(work());
                repaint.request_repaint();
            }) {
            Ok(handle) => {
                self.job = Some(Job {
                    document,
                    receive,
                    write: writing.then_some(handle),
                })
            }
            Err(error) => self.message = Some(error.to_string()),
        }
    }

    fn change(&mut self, ctx: &egui::Context, data: &Path, change: Change) {
        let Some(book) = &self.book else {
            return;
        };
        if self.job.is_some() {
            return;
        }
        let (id, key, data) = (book.id, book.key.clone(), data.to_owned());
        self.run(ctx, id, true, move || {
            notebook::update(&data, &key, change).map(Outcome::Saved)
        });
    }

    pub(super) fn finish(&mut self) {
        if let Some(write) = self.job.take().and_then(|job| job.write) {
            let _ = write.join();
        }
    }
}

impl ReaderApp {
    pub(super) fn remember_notebook_marks(&mut self, document: DocumentId, path: PathBuf) {
        if self.notebook.active == Some(document) {
            self.notebook.load_attempted = false;
            self.notebook.confirm_mark_delete = None;
        }
        if self.notebook.mark_indexed != Some(document)
            && Format::detect(&path).ok() == Some(Format::Pdf)
        {
            let stamp = self
                .tabs
                .iter()
                .find(|tab| tab.reader.id == document)
                .and_then(|tab| tab.reader.state.document.as_ref())
                .and_then(|doc| doc.info.source_fingerprint);
            self.notebook.mark_request = Some((document, path, stamp));
        }
    }

    pub(super) fn poll_notebook(&mut self, ctx: &egui::Context) {
        let active = (!self.home)
            .then(|| self.tabs.get(self.active))
            .flatten()
            .filter(|tab| tab.reader.state.document.is_some());
        let id = active.map(|tab| tab.reader.id);
        let state = &mut self.notebook;
        let mut reload = None;
        if state.active != id {
            state.active = id;
            state.book = None;
            state.query.clear();
            state.matches = None;
            state.notes_page = 0;
            state.load_attempted = false;
            state.message = None;
            state.confirm_delete = None;
            state.confirm_mark_delete = None;
        }
        if let Some(job) = &state.job {
            let result = match job.receive.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(io::Error::other(
                    "Se interrumpió el trabajo del cuaderno.",
                ))),
            };
            if let Some(result) = result {
                let document = job.document;
                state.job = None;
                match result {
                    Ok(Outcome::Saved(notes)) => {
                        if let Some(book) = state.book.as_mut().filter(|book| book.id == document) {
                            book.notes = notes;
                            state.matches = None;
                            state.message = Some("Cuaderno actualizado.".into());
                        }
                        if state
                            .draft
                            .as_ref()
                            .is_some_and(|draft| draft.document == document)
                        {
                            state.draft = None;
                        }
                        state.confirm_delete = None;
                    }
                    Ok(Outcome::Loaded(book)) if id == Some(document) => {
                        state.book = Some(book);
                        state.matches = None;
                        state.confirm_mark_delete = None;
                    }
                    Ok(Outcome::Exported) => self.notice = Some("Cuaderno exportado.".into()),
                    Ok(Outcome::Indexed) => state.mark_indexed = Some(document),
                    Ok(Outcome::MarksChanged) => {
                        self.notice = Some("Marca actualizada.".into());
                        reload = Some(document);
                    }
                    Ok(Outcome::Restored(count)) => {
                        self.notice = Some(format!("Se recuperaron {count} marcas."));
                        if id == Some(document) {
                            reload = Some(document);
                        }
                    }
                    Err(error) if id == Some(document) => state.message = Some(error.to_string()),
                    Err(error) => self.notice = Some(error.to_string()),
                    _ => {}
                }
            }
        }
        if state.job.is_none()
            && let Some((document, key, title, entry, dialog)) = &mut state.export
            && let Some((_, files)) = dialog.poll(ctx)
        {
            let (id, key, title, entry) = (*document, key.clone(), title.clone(), *entry);
            state.export = None;
            if let Some(path) = files.into_iter().next() {
                let data = self.app_paths.data.clone();
                state.run(ctx, id, true, move || {
                    notebook::export(&data, &key, &path, &title, entry)?;
                    Ok(Outcome::Exported)
                });
            }
        }
        let visible = if self.fullscreen {
            self.fullscreen_sidebar
        } else {
            self.preferences.sidebar_open
        };
        if visible
            && self.sidebar == Sidebar::Notebook
            && state.job.is_none()
            && !state.load_attempted
            && let Some(tab) = active
        {
            state.load_attempted = true;
            let id = tab.reader.id;
            let path = tab.reader.path.clone();
            let stamp = tab
                .reader
                .state
                .document
                .as_ref()
                .and_then(|doc| doc.info.source_fingerprint);
            if Format::detect(&path).ok() != Some(Format::Pdf) {
                return;
            }
            let data = self.app_paths.data.clone();
            state.run(ctx, id, false, move || {
                let key = notebook::identify_open_document(&path, stamp)?;
                let notes = notebook::load(&data, &key)?;
                let marks = notebook::pdf_mark_recovery(&data, &path, &key, false);
                let local_marks = notebook::pdf_marks(&data, &path).unwrap_or_default();
                Ok(Outcome::Loaded(Book {
                    id,
                    key,
                    notes,
                    marks,
                    local_marks,
                }))
            });
        }
        if state.job.is_none()
            && let Some((document, path, stamp)) = state.mark_request.take()
            && state.mark_indexed != Some(document)
        {
            let data = self.app_paths.data.clone();
            state.run(ctx, document, true, move || {
                let key = notebook::identify_open_document(&path, stamp)?;
                notebook::pdf_mark_recovery(&data, &path, &key, false)?;
                Ok(Outcome::Indexed)
            });
        }
        if let Some(document) = reload
            && self
                .tabs
                .get(self.active)
                .is_some_and(|tab| tab.reader.id == document)
        {
            self.reload_active_tab();
        }
    }

    pub(super) fn open_notebook(&mut self) {
        self.sidebar = Sidebar::Notebook;
        if self.fullscreen {
            self.fullscreen_sidebar = true;
        } else {
            self.preferences.sidebar_open = true;
            self.schedule_save();
        }
    }

    pub(super) fn draft_note(&mut self, document: DocumentId, note: Note) {
        if !self.tabs.iter().any(|tab| {
            tab.reader.id == document && Format::detect(&tab.reader.path).ok() == Some(Format::Pdf)
        }) {
            self.notice = Some("Para guardar esta entrada, abre un PDF.".into());
            return;
        }
        if self.notebook.draft.is_some() {
            self.notice = Some("Guarda o descarta la nota que estás editando.".into());
        } else {
            self.notebook.focus_draft = true;
            self.notebook.draft = Some(Draft {
                document,
                original: None,
                note,
            });
        }
        self.open_notebook();
    }

    pub(super) fn notebook_panel(&mut self, ui: &mut egui::Ui) {
        let Some(tab) = self.tabs.get(self.active) else {
            ui.label("Abre un documento para ver su cuaderno.");
            return;
        };
        let Some(document) = &tab.reader.state.document else {
            ui.label("Esperando documento…");
            return;
        };
        if Format::detect(&tab.reader.path).ok() != Some(Format::Pdf) {
            ui.label("Abre un PDF para usar el cuaderno.");
            return;
        }
        let id = tab.reader.id;
        ui.heading("Cuaderno");
        let data = &self.app_paths.data;
        let state = &mut self.notebook;
        if state.job.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Actualizando el cuaderno…");
            });
        }
        if let Some(message) = &state.message {
            ui.label(message);
        }
        let Some(book) = state.book.as_ref().filter(|book| book.id == id) else {
            if state.job.is_none() && ui.button("Cargar cuaderno").clicked() {
                state.load_attempted = false;
                ui.ctx().request_repaint();
            }
            return;
        };
        let busy = state.job.is_some() || state.export.is_some();
        let mut save = None;
        let mut recover = false;
        match &book.marks {
            Ok(MarkRecovery::Available(count)) => {
                ui.label(format!(
                    "Hay {count} marcas de este mismo PDF en el perfil."
                ));
                recover = ui
                    .add_enabled(
                        !busy && state.draft.is_none(),
                        egui::Button::new("Recuperar marcas"),
                    )
                    .clicked();
            }
            Err(error) => {
                ui.label(error.to_string());
            }
            _ => {}
        }
        let mut export = None;
        let mut jump = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy && state.draft.is_none(),
                    egui::Button::new("Nueva nota"),
                )
                .clicked()
            {
                state.focus_draft = true;
                state.draft = Some(Draft {
                    document: id,
                    original: None,
                    note: Note::page(NoteAnchor::page(document.page)),
                });
            }
            if ui
                .add_enabled(
                    !busy && !book.notes.is_empty(),
                    egui::Button::new("Exportar Markdown…"),
                )
                .clicked()
            {
                export = Some(None);
            }
            if ui
                .add_enabled(!busy, egui::Button::new("Recargar"))
                .clicked()
            {
                state.load_attempted = false;
                ui.ctx().request_repaint();
            }
        });
        if let Some(draft) = state.draft.as_mut() {
            ui.separator();
            if draft.document == id {
                ui.strong(draft.note.anchor.label());
                if !draft.note.excerpt.is_empty() {
                    egui::ScrollArea::vertical()
                        .id_salt("draft-excerpt")
                        .max_height(85.0)
                        .show(ui, |ui| {
                            ui.label(&draft.note.excerpt);
                        });
                }
                let editor = ui.add_enabled(
                    !busy,
                    egui::TextEdit::multiline(&mut draft.note.comment)
                        .hint_text("Escribe una nota")
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if !busy && std::mem::take(&mut state.focus_draft) {
                    editor.request_focus();
                }
                let keyboard_save = editor.has_focus()
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                    });
                let mut discard = false;
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !busy && draft.note.is_valid(),
                            egui::Button::new("Guardar nota"),
                        )
                        .clicked()
                        || (!busy && draft.note.is_valid() && keyboard_save)
                    {
                        save = Some(match &draft.original {
                            Some(original) => Change::Edit {
                                original: original.clone(),
                                replacement: draft.note.clone(),
                            },
                            None => Change::Add(draft.note.clone()),
                        });
                    }
                    discard = ui
                        .add_enabled(!busy, egui::Button::new("Descartar"))
                        .clicked();
                });
                if discard {
                    state.draft = None;
                }
            } else {
                ui.label("Hay una nota abierta en otra pestaña.");
                if ui
                    .add_enabled(!busy, egui::Button::new("Descartar borrador"))
                    .clicked()
                {
                    state.draft = None;
                }
            }
        }
        let mut edit_mark = None;
        if !book.local_marks.is_empty() {
            let editable = !busy
                && state.draft.is_none()
                && tab
                    .reader
                    .state
                    .details
                    .as_ref()
                    .is_some_and(|info| info.can_annotate && info.can_copy);
            ui.collapsing(
                format!("Marcas del PDF ({})", book.local_marks.len()),
                |ui| {
                    let height = ui.spacing().interact_size.y + 3.0;
                    egui::ScrollArea::vertical()
                        .id_salt("notebook-marks")
                        .max_height(180.0)
                        .show_rows(ui, height, book.local_marks.len(), |ui, visible| {
                            for index in visible {
                                let mark = &book.local_marks[index];
                                ui.push_id(index, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new("●").color(
                                            egui::Color32::from_rgb(
                                                mark.color[0],
                                                mark.color[1],
                                                mark.color[2],
                                            ),
                                        ));
                                        let kind = match mark.kind {
                                            crate::document::MarkupKind::Highlight => "Resaltado",
                                            crate::document::MarkupKind::Underline => "Subrayado",
                                        };
                                        if ui
                                            .link(format!("{kind}, pág. {}", mark.page + 1))
                                            .clicked()
                                        {
                                            jump = Some(NoteAnchor::page(mark.page));
                                        }
                                        ui.menu_button("…", |ui| {
                                            for (name, color) in [
                                                ("Amarillo", [255, 230, 50]),
                                                ("Verde", [115, 200, 112]),
                                                ("Azul", [110, 160, 246]),
                                                ("Rosa", [238, 135, 170]),
                                            ] {
                                                if ui
                                                    .add_enabled(editable, egui::Button::new(name))
                                                    .clicked()
                                                {
                                                    edit_mark = Some((index, Some(color)));
                                                    ui.close();
                                                }
                                            }
                                            if ui
                                                .add_enabled(
                                                    editable,
                                                    egui::Button::new("Eliminar marca…"),
                                                )
                                                .clicked()
                                            {
                                                state.confirm_mark_delete = Some(index);
                                                ui.close();
                                            }
                                        });
                                    });
                                });
                            }
                        });
                    if let Some(index) = state.confirm_mark_delete {
                        ui.label("¿Eliminar esta marca?");
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(editable, egui::Button::new("Eliminar"))
                                .clicked()
                            {
                                edit_mark = Some((index, None));
                                state.confirm_mark_delete = None;
                            }
                            if ui.button("Cancelar").clicked() {
                                state.confirm_mark_delete = None;
                            }
                        });
                    }
                },
            );
        }
        if book.notes.is_empty() {
            ui.add_space(6.0);
            ui.label("Añade una nota o guarda una selección del PDF.");
        } else {
            ui.separator();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut state.query)
                        .hint_text("Buscar en el cuaderno")
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                state.matches = None;
                state.notes_page = 0;
            }
            const PER_PAGE: usize = 24;
            let found = state.matches.get_or_insert_with(|| {
                let query = state.query.trim().to_lowercase();
                book.notes
                    .iter()
                    .enumerate()
                    .rev()
                    .filter(|(_, note)| {
                        query.is_empty()
                            || note.excerpt.to_lowercase().contains(&query)
                            || note.comment.to_lowercase().contains(&query)
                            || note.anchor.label().to_lowercase().contains(&query)
                    })
                    .map(|(index, _)| index)
                    .collect()
            });
            let last = found.len().saturating_sub(1) / PER_PAGE;
            state.notes_page = state.notes_page.min(last);
            if last > 0 {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(state.notes_page > 0, egui::Button::new("Anterior"))
                        .clicked()
                    {
                        state.notes_page -= 1;
                    }
                    ui.small(format!("{} de {}", state.notes_page + 1, last + 1));
                    if ui
                        .add_enabled(state.notes_page < last, egui::Button::new("Siguiente"))
                        .clicked()
                    {
                        state.notes_page += 1;
                    }
                });
            }
            egui::ScrollArea::vertical()
                .id_salt(("notebook-entries", state.notes_page))
                .show(ui, |ui| {
                    for &index in found
                        .iter()
                        .skip(state.notes_page * PER_PAGE)
                        .take(PER_PAGE)
                    {
                        let note = &book.notes[index];
                        ui.push_id(note.id, |ui| {
                            ui.separator();
                            if ui
                                .link(note.anchor.label())
                                .on_hover_text("Ir al pasaje")
                                .clicked()
                            {
                                jump = Some(note.anchor);
                            }
                            let preview = if note.excerpt.is_empty() {
                                &note.comment
                            } else {
                                &note.excerpt
                            };
                            let end = preview
                                .char_indices()
                                .nth(170)
                                .map_or(preview.len(), |(index, _)| index);
                            let text = &preview[..end];
                            let line = if end < preview.len() {
                                format!("{text}…")
                            } else {
                                text.to_owned()
                            };
                            ui.label(if note.kind == NoteKind::Latex {
                                egui::RichText::new(line).monospace()
                            } else {
                                egui::RichText::new(line)
                            });
                            if !note.excerpt.is_empty() && !note.comment.is_empty() {
                                ui.small(note.comment.chars().take(100).collect::<String>());
                            }
                            ui.menu_button("Acciones", |ui| {
                                if ui.button("Copiar").clicked() {
                                    ui.ctx().copy_text(preview.clone());
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        !busy && state.draft.is_none(),
                                        egui::Button::new("Editar nota"),
                                    )
                                    .clicked()
                                {
                                    state.focus_draft = true;
                                    state.draft = Some(Draft {
                                        document: id,
                                        original: Some(note.clone()),
                                        note: note.clone(),
                                    });
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(!busy, egui::Button::new("Exportar entrada…"))
                                    .clicked()
                                {
                                    export = Some(Some(note.id));
                                    ui.close();
                                }
                                if ui
                                    .add_enabled(
                                        !busy && state.draft.is_none(),
                                        egui::Button::new("Eliminar…"),
                                    )
                                    .clicked()
                                {
                                    state.confirm_delete = Some(note.id);
                                    ui.close();
                                }
                            });
                            if state.confirm_delete == Some(note.id) {
                                ui.horizontal_wrapped(|ui| {
                                    if ui
                                        .add_enabled(!busy, egui::Button::new("Eliminar entrada"))
                                        .clicked()
                                    {
                                        save = Some(Change::Delete(note.clone()));
                                    }
                                    if ui.button("Cancelar").clicked() {
                                        state.confirm_delete = None;
                                    }
                                });
                            }
                        });
                    }
                });
            if found.is_empty() {
                ui.label("Sin coincidencias.");
            }
        }
        if let Some((index, color)) = edit_mark {
            let key = book.key.clone();
            let data = data.clone();
            let path = tab.reader.path.clone();
            let expected = book.local_marks.clone();
            state.run(ui.ctx(), id, true, move || {
                notebook::edit_pdf_mark(&data, &path, &key, expected, index, color)?;
                Ok(Outcome::MarksChanged)
            });
        } else if recover {
            let key = book.key.clone();
            let path = tab.reader.path.clone();
            let data = data.clone();
            state.run(
                ui.ctx(),
                id,
                true,
                move || match notebook::pdf_mark_recovery(&data, &path, &key, true)? {
                    MarkRecovery::Restored(count) => Ok(Outcome::Restored(count)),
                    _ => Err(io::Error::other(
                        "No hay marcas que recuperar en este archivo.",
                    )),
                },
            );
        } else if let Some(change) = save {
            state.change(ui.ctx(), data, change);
        } else if let Some(entry) = export {
            let name = tab
                .reader
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("Cuaderno")
                .to_owned();
            let key = book.key.clone();
            let dialog = PendingDialog::markdown(id, format!("{name} - Cuaderno.md"));
            state.export = Some((id, key, name, entry, dialog));
        }
        if let Some(anchor) = jump {
            let tab = &mut self.tabs[self.active];
            tab.go_to(anchor.first_page, true);
            if let Some(region) = anchor.region {
                let bounds = region.rotated(tab.reader.state.rotation);
                tab.reader.pending_position = crate::reading_position::ReadingPosition::centered(
                    anchor.first_page,
                    (bounds.left + bounds.right) * 0.5,
                    (bounds.top + bounds.bottom) * 0.5,
                );
            }
            self.schedule_save();
            ui.ctx().request_repaint();
        }
    }
}
