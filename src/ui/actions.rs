//! Desktop actions coordination. Reading rules live in crabpdf-application.
use super::*;

impl ReaderApp {
    pub(super) fn enabled(&self, action: Action) -> bool {
        if self.home
            && matches!(
                action,
                Action::Close
                    | Action::CloseOthers
                    | Action::CloseLeft
                    | Action::CloseRight
                    | Action::MoveTabLeft
                    | Action::MoveTabRight
            )
        {
            return false;
        }
        if matches!(
            action,
            Action::Close
                | Action::CloseAll
                | Action::CloseOthers
                | Action::CloseLeft
                | Action::CloseRight
        ) {
            return !self.tabs.is_empty();
        }
        if action.needs_document() && self.state().document.is_none() {
            return false;
        }
        match action {
            Action::Highlight | Action::Underline => {
                self.tabs.get(self.active).is_some_and(|tab| {
                    tab.reader.selection.range().is_some() && !tab.reader.marking
                }) && self
                    .state()
                    .details
                    .as_ref()
                    .is_some_and(|details| details.can_annotate && details.can_copy)
            }
            Action::UndoMarkup => self
                .tabs
                .get(self.active)
                .is_some_and(|tab| tab.reader.local_marks > 0 && !tab.reader.marking),
            Action::Reopen => !self.closed.is_empty(),
            Action::Open => self.worker.is_some() && self.dialog.is_none(),
            Action::Previous | Action::First => {
                self.state().document.as_ref().is_some_and(|d| d.page > 0)
            }
            Action::Next | Action::Last => self
                .state()
                .document
                .as_ref()
                .is_some_and(|d| d.page + 1 < d.info.page_count),
            Action::Back => self.state().can_back(),
            Action::Forward => self.state().can_forward(),
            Action::Copy => {
                self.tabs
                    .get(self.active)
                    .is_some_and(|t| t.reader.selection.range().is_some())
                    && self
                        .state()
                        .details
                        .as_ref()
                        .is_some_and(|details| details.can_copy)
            }
            Action::FindNext | Action::FindPrevious => self
                .tabs
                .get(self.active)
                .is_some_and(|t| !t.reader.search.hits.is_empty()),
            Action::NextTab | Action::PreviousTab => !self.tabs.is_empty(),
            Action::MoveTabLeft | Action::MoveTabRight => self.tabs.len() > 1,
            _ => true,
        }
    }
    pub fn execute(&mut self, action: Action, ctx: &egui::Context) {
        if !self.enabled(action) {
            return;
        }
        if matches!(
            action,
            Action::Previous
                | Action::Next
                | Action::First
                | Action::Last
                | Action::GoTo
                | Action::Back
                | Action::Forward
                | Action::FindNext
                | Action::FindPrevious
                | Action::ScrollUp
                | Action::ScrollDown
                | Action::ScrollLeft
                | Action::ScrollRight
                | Action::ScrollUpPage
                | Action::ScrollDownPage
                | Action::ScrollLeftPage
                | Action::ScrollRightPage
                | Action::ScrollUpHalf
                | Action::ScrollDownHalf
                | Action::ZoomIn
                | Action::ZoomOut
                | Action::ActualSize
                | Action::FitWidth
                | Action::FitHeight
                | Action::FitPage
                | Action::RotateLeft
                | Action::RotateRight
                | Action::Single
                | Action::Facing
                | Action::Book
                | Action::Continuous
                | Action::VerticalReading
                | Action::Manga
        ) && let Some(tab) = self.tabs.get_mut(self.active)
        {
            tab.reader.pending_position = None;
        }
        match action {
            Action::Highlight | Action::Underline => {
                let tab = &mut self.tabs[self.active];
                if let Some(selection) = tab.reader.selection.range()
                    && let Some(worker) = &self.worker
                {
                    let mark = crabpdf_core::document::TextMarkup {
                        page: selection.page,
                        start: selection.start,
                        end: selection.end,
                        kind: if action == Action::Highlight {
                            crabpdf_core::document::MarkupKind::Highlight
                        } else {
                            crabpdf_core::document::MarkupKind::Underline
                        },
                        color: self.preferences.markup_color,
                    };
                    match worker.mark(tab.reader.id, mark) {
                        Ok(()) => tab.reader.marking = true,
                        Err(error) => self.error(error),
                    }
                }
            }
            Action::UndoMarkup => {
                let tab = &mut self.tabs[self.active];
                if let Some(worker) = &self.worker {
                    match worker.undo_mark(tab.reader.id) {
                        Ok(()) => tab.reader.marking = true,
                        Err(error) => self.error(error),
                    }
                }
            }
            Action::Open => self.open_dialog(),
            Action::Home => {
                if self.presentation {
                    self.leave_presentation(ctx);
                }
                self.home = true;
            }
            Action::Maximize => {
                let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
            }
            Action::SaveAs => self.save_dialog(),
            Action::Close => self.close_index(self.active),
            Action::CloseAll => {
                while !self.tabs.is_empty() {
                    self.close_index(self.tabs.len() - 1);
                }
            }
            Action::CloseOthers => {
                let keep = self.tabs[self.active].reader.id;
                for index in (0..self.tabs.len()).rev() {
                    if self.tabs[index].reader.id != keep {
                        self.close_index(index);
                    }
                }
            }
            Action::CloseLeft | Action::CloseRight => {
                let active = self.active;
                for index in (0..self.tabs.len()).rev() {
                    if action == Action::CloseLeft && index < active
                        || action == Action::CloseRight && index > active
                    {
                        self.close_index(index);
                    }
                }
            }
            Action::Reopen => {
                if let Some(saved) = self.closed.pop() {
                    self.open_document_inner(saved.path.clone(), Some(saved), true);
                }
            }
            Action::Reload => {
                let saved = self.tabs[self.active].stored();
                let path = self.tabs[self.active].reader.path.clone();
                let index = self.active;
                self.close_index(index);
                self.open_document(path, saved);
            }
            Action::NewWindow => {
                if let Err(error) = platform::new_window_with_paths(None, &self.app_paths) {
                    self.error(error);
                }
            }
            Action::DuplicateTab => {
                if let Some(saved) = self.tabs[self.active].stored() {
                    self.open_document_inner(saved.path.clone(), Some(saved), true);
                }
            }
            Action::CopyPath => {
                ctx.copy_text(self.tabs[self.active].reader.path.display().to_string())
            }
            Action::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::DuplicateWindow => {
                if let Some(saved) = self.tabs[self.active].stored()
                    && let Err(error) =
                        platform::document_window_with_paths(&saved, &self.app_paths)
                {
                    self.error(error);
                }
            }
            Action::Reveal => {
                let path = self.tabs[self.active].reader.path.clone();
                if let Err(e) = platform::reveal(&path) {
                    self.error(e);
                }
            }
            Action::Properties => self.properties_open = true,
            Action::Previous => self.navigate(false),
            Action::Next => self.navigate(true),
            Action::First => self.tabs[self.active].go_to(0, true),
            Action::Last => {
                let tab = &mut self.tabs[self.active];
                let count = tab
                    .reader
                    .state
                    .document
                    .as_ref()
                    .map_or(1, |d| d.info.page_count);
                tab.go_to(count - 1, true);
            }
            Action::GoTo => {
                self.go_to_value = self.state().document.as_ref().map_or(1, |d| d.page + 1);
                self.go_to_open = true;
            }
            Action::Back | Action::Forward => {
                let tab = &mut self.tabs[self.active];
                tab.reader.selection.cancel_pending();
                tab.reader.state.history_navigate(action == Action::Forward);
            }
            Action::ScrollUp
            | Action::ScrollDown
            | Action::ScrollLeft
            | Action::ScrollRight
            | Action::ScrollUpPage
            | Action::ScrollDownPage
            | Action::ScrollLeftPage
            | Action::ScrollRightPage
            | Action::ScrollUpHalf
            | Action::ScrollDownHalf => self.scroll(action),
            Action::ZoomIn | Action::ZoomOut => {
                let zoom = self.effective_zoom();
                let tab = &mut self.tabs[self.active];
                let anchor = tab.anchor(None);
                let before = tab.reader.state.zoom;
                tab.reader.state.change_zoom(action == Action::ZoomIn, zoom);
                if tab.reader.state.zoom != before {
                    tab.view.zoom_anchor = anchor;
                    if anchor.is_some() {
                        tab.reader.state.scroll_to_page = false;
                    }
                }
            }
            Action::ActualSize => self.set_zoom(Zoom::Percent(100)),
            Action::FitWidth => self.set_zoom(Zoom::FitWidth),
            Action::FitHeight => self.set_zoom(Zoom::FitHeight),
            Action::FitPage => self.set_zoom(Zoom::FitPage),
            Action::CustomZoom => {
                self.zoom_value = self.effective_zoom().round().clamp(8.0, 6400.0) as u16;
                self.zoom_open = true;
            }
            Action::RotateLeft | Action::RotateRight => self.tabs[self.active]
                .reader
                .state
                .rotate(action == Action::RotateRight),
            Action::Single | Action::Facing | Action::Book => {
                let tab = &mut self.tabs[self.active];
                tab.reader.state.layout = match action {
                    Action::Facing => LayoutMode::Facing,
                    Action::Book => LayoutMode::Book,
                    _ => LayoutMode::Single,
                };
                tab.reader.state.invalidate();
                tab.reader.state.scroll_to_page = true;
            }
            Action::Continuous => {
                let tab = &mut self.tabs[self.active];
                tab.reader.state.continuous = !tab.reader.state.continuous;
                tab.reader.state.invalidate();
                tab.reader.state.scroll_to_page = true;
            }
            Action::Manga => {
                self.tabs[self.active].reader.state.manga =
                    !self.tabs[self.active].reader.state.manga
            }
            Action::VerticalReading => {
                let tab = &mut self.tabs[self.active];
                tab.reader.state.layout = LayoutMode::Single;
                tab.reader.state.continuous = true;
                tab.reader.state.set_zoom(Zoom::FitWidth);
                tab.reader.state.invalidate();
                tab.reader.state.scroll_to_page = true;
                tab.view.scroll.x = 0.0;
            }
            Action::Find => {
                let tab = &mut self.tabs[self.active];
                tab.view.search.open = true;
                tab.view.search.focus = true;
            }
            Action::FindNext => self.step_search(true),
            Action::FindPrevious => self.step_search(false),
            Action::Copy => {
                if let Some(text) = self.selected_text() {
                    ctx.copy_text(text);
                }
            }
            Action::SelectAll => {
                let tab = &mut self.tabs[self.active];
                let id = tab.reader.id;
                if let Some(page) = tab.reader.request_select_all()
                    && let Some(content) = self.need_content(id, page)
                {
                    self.tabs[self.active]
                        .reader
                        .accept_content(id, page, &content);
                }
            }
            Action::ToggleSidebar => {
                if self.fullscreen {
                    self.fullscreen_sidebar = !self.fullscreen_sidebar;
                } else {
                    self.preferences.sidebar_open = !self.preferences.sidebar_open;
                    self.save_preferences();
                }
            }
            Action::Thumbnails | Action::Outline | Action::Favorites => {
                if self.fullscreen {
                    self.fullscreen_sidebar = true;
                } else {
                    self.preferences.sidebar_open = true;
                }
                self.sidebar = match action {
                    Action::Outline => Sidebar::Outline,
                    Action::Favorites => Sidebar::Favorites,
                    _ => Sidebar::Thumbnails,
                };
                self.save_preferences();
            }
            Action::ToggleFavorite => {
                if let Some(document) = &self.state().document {
                    self.favorite_action(FavoriteAction::Toggle {
                        page: document.page,
                        label: format!("Página {}", document.page + 1),
                    });
                }
            }
            Action::Theme => self.toggle_theme(ctx),
            Action::Invert | Action::Grayscale => {
                let tab = &mut self.tabs[self.active];
                let selected = if action == Action::Invert {
                    crabpdf_core::document::PageColors::Inverted
                } else {
                    crabpdf_core::document::PageColors::Grayscale
                };
                tab.reader.state.colors = if tab.reader.state.colors == selected {
                    crabpdf_core::document::PageColors::Original
                } else {
                    selected
                };
                tab.reader.state.invalidate();
            }
            Action::Fullscreen => {
                if self.presentation {
                    self.leave_presentation(ctx);
                    self.fullscreen = false;
                } else {
                    self.fullscreen = !self.fullscreen;
                }
                self.fullscreen_sidebar = false;
                self.fullscreen_controls = None;
                self.sync_fullscreen(ctx);
            }
            Action::Presentation => {
                if self.presentation {
                    self.leave_presentation(ctx);
                } else {
                    self.presentation_previous_fullscreen = Some(self.fullscreen);
                    self.presentation = true;
                    self.fullscreen = true;
                    self.sync_fullscreen(ctx);
                }
            }
            Action::Toolbar => self.toolbar_visible = !self.toolbar_visible,
            Action::MenuBar => self.menu_visible = !self.menu_visible,
            Action::Palette => {
                self.palette_query.clear();
                self.palette_open = true;
                self.palette_index = 0;
                self.palette_focus = true;
                self.palette_reveal = true;
            }
            Action::Settings => self.settings_open = true,
            Action::NextTab | Action::PreviousTab => {
                let count = self.tabs.len() + 1;
                let current = if self.home { 0 } else { self.active + 1 };
                let next = (current
                    + if action == Action::NextTab {
                        1
                    } else {
                        count - 1
                    })
                    % count;
                self.home = next == 0;
                if next > 0 {
                    self.active = next - 1;
                }
                self.reveal_tab = true;
            }
            Action::MoveTabLeft => {
                if self.active > 0 {
                    self.tabs.swap(self.active, self.active - 1);
                    self.active -= 1;
                }
            }
            Action::MoveTabRight => {
                if self.active + 1 < self.tabs.len() {
                    self.tabs.swap(self.active, self.active + 1);
                    self.active += 1;
                }
            }
            Action::ClearHistory => {
                self.preferences.recent.clear();
                self.preferences.library.clear();
                self.covers.clear();
                self.cover_errors.clear();
                self.save_preferences();
            }
        }
        self.schedule_save();
        ctx.request_repaint();
    }
}
