//! Desktop input coordination. Reading rules live in crabpdf-application.
use super::*;

impl ReaderApp {
    pub(super) fn effective_zoom(&self) -> f32 {
        if self.home {
            return 100.0;
        }
        self.tabs
            .get(self.active)
            .and_then(Tab::effective_zoom)
            .unwrap_or(100.0)
    }
    pub(super) fn leave_presentation(&mut self, ctx: &egui::Context) {
        self.presentation = false;
        self.fullscreen = self
            .presentation_previous_fullscreen
            .take()
            .unwrap_or(false);
        self.sync_fullscreen(ctx);
    }
    pub(super) fn scroll(&mut self, action: Action) {
        let tab = &mut self.tabs[self.active];
        let horizontal = matches!(
            action,
            Action::ScrollLeft
                | Action::ScrollRight
                | Action::ScrollLeftPage
                | Action::ScrollRightPage
        );
        let forward = matches!(
            action,
            Action::ScrollDown
                | Action::ScrollRight
                | Action::ScrollDownPage
                | Action::ScrollRightPage
                | Action::ScrollDownHalf
        );
        let fits_horizontally = tab.view.view_layout.as_ref().is_some_and(|(_, layout)| {
            let Some(document) = &tab.reader.state.document else {
                return false;
            };
            let spread = tab
                .reader
                .state
                .layout
                .spread(document.page, document.info.page_count);
            let mut left = f32::INFINITY;
            let mut right = f32::NEG_INFINITY;
            for placement in &layout.placements {
                if spread.contains(&placement.page) {
                    left = left.min(placement.left);
                    right = right.max(placement.left + placement.size.width);
                }
            }
            left.is_finite() && right - left <= tab.view.viewport.x + 1.0
        });
        if horizontal
            && (matches!(tab.reader.state.zoom, Zoom::FitWidth | Zoom::FitPage)
                || fits_horizontally)
        {
            self.navigate(forward);
            return;
        }
        let screen = if horizontal {
            tab.view.viewport.x
        } else {
            tab.view.viewport.y
        };
        let step = match action {
            Action::ScrollUpPage
            | Action::ScrollDownPage
            | Action::ScrollLeftPage
            | Action::ScrollRightPage => screen * 0.9,
            Action::ScrollUpHalf | Action::ScrollDownHalf => screen * 0.5,
            _ => 40.0,
        };
        let content = tab.view.view_layout.as_ref().map_or(screen, |(_, layout)| {
            if horizontal {
                layout.width
            } else {
                layout.height
            }
        });
        let offset = if horizontal {
            &mut tab.view.scroll.x
        } else {
            &mut tab.view.scroll.y
        };
        let old = *offset;
        let delta = if forward { step } else { -step };
        let target = (old + delta).clamp(0.0, (content - screen).max(0.0));
        tab.view.scroll_motion = None;
        tab.view.navigation_wait = None;
        if !horizontal && !self.preferences.reduce_motion {
            tab.view.key_scroll.nudge(old, delta, content - screen);
        } else {
            *offset = target;
        }
        if (target - old).abs() < 0.1 && (horizontal || !tab.reader.state.continuous) {
            self.navigate(forward);
            if !forward && !horizontal {
                self.tabs[self.active].view.scroll_bottom = true;
            }
        }
    }
    pub(super) fn update_arrow_scroll(&mut self, ctx: &egui::Context, keyboard: bool) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let (focused, plain, direction, tap) = ctx.input(|input| {
            let direction = i8::from(input.key_down(egui::Key::ArrowDown))
                - i8::from(input.key_down(egui::Key::ArrowUp));
            let tapped = |key| input.events.iter().any(|event| matches!(event,
                egui::Event::Key { key: pressed, pressed: true, repeat: false, .. } if *pressed == key));
            (input.focused, input.modifiers.is_none(), direction,
             i8::from(tapped(egui::Key::ArrowDown)) - i8::from(tapped(egui::Key::ArrowUp)))
        });
        let max = tab.view.view_layout.as_ref().map_or(0.0, |(_, layout)| {
            (layout.height - tab.view.viewport.y).max(0.0)
        });
        let intent = if direction == 0 { tap } else { direction };
        let page_edge = !tab.reader.state.continuous
            && intent != 0
            && if intent > 0 {
                tab.view.scroll.y >= max - 0.1
            } else {
                tab.view.scroll.y <= 0.1
            };
        if keyboard
            || !focused
            || !plain
            || self.home
            || self.presentation
            || tab.reader.state.document.is_none()
            || page_edge
        {
            tab.view.key_scroll.cancel();
            return;
        }
        tab.view.key_scroll.input(direction, tap);
        // Consume presses/repeats, but retain key-down state for the next frame.
        ctx.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
            input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
        });
        if tab.view.key_scroll.active() {
            tab.reader.pending_position = None;
            tab.reader.state.scroll_to_page = false;
            tab.view.scroll_motion = None;
            tab.view.navigation_wait = None;
            ctx.request_repaint();
        }
    }
    pub(super) fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.view.key_scroll.cancel();
            }
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.tab_drag.take().is_some() {
                return;
            }
            let dismissing = self.palette_open
                || self.go_to_open
                || self.zoom_open
                || self.properties_open
                || self.settings_open
                || !self.home
                    && self
                        .tabs
                        .get(self.active)
                        .is_some_and(|tab| tab.reader.password_needed || tab.view.search.open);
            self.palette_open = false;
            self.go_to_open = false;
            self.zoom_open = false;
            self.properties_open = false;
            self.settings_open = false;
            if !dismissing && self.presentation {
                self.leave_presentation(ctx);
            } else if !dismissing && self.fullscreen {
                self.fullscreen = false;
                self.presentation = false;
                self.sync_fullscreen(ctx);
            }
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.reader.selection.clear();
                tab.view.search.open = false;
                tab.reader.password_needed = false;
                tab.view.password.clear();
            }
            ctx.memory_mut(|memory| {
                if let Some(id) = memory.focused() {
                    memory.surrender_focus(id);
                }
            });
        }
        // Buttons can retain focus after a click. Only text fields, dialogs and
        // menus own reader keys; otherwise the arrows must still move the PDF.
        let keyboard = ctx.text_edit_focused()
            || self.palette_open
            || self.go_to_open
            || self.zoom_open
            || self.properties_open
            || self.settings_open
            || self
                .tabs
                .get(self.active)
                .is_some_and(|tab| tab.reader.password_needed)
            || egui::Popup::is_any_open(ctx);
        let (wheel_zoom, pointer) =
            ctx.input(|input| (input.zoom_delta(), input.pointer.hover_pos()));
        if !keyboard
            && self.state().document.is_some()
            && wheel_zoom != 1.0
            && let Some(tab) = self.tabs.get(self.active)
            && pointer
                .zip(tab.view.viewport_rect)
                .is_some_and(|(pointer, rect)| rect.contains(pointer))
        {
            let percent = (self.effective_zoom() * wheel_zoom)
                .round()
                .clamp(8.0, 6400.0) as u16;
            self.tabs[self.active].zoom_at(Zoom::Percent(percent), pointer);
            self.schedule_save();
        }
        let bindings = [
            (egui::Modifiers::COMMAND, egui::Key::O, Action::Open),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::H,
                Action::Home,
            ),
            (egui::Modifiers::COMMAND, egui::Key::W, Action::Close),
            (egui::Modifiers::COMMAND, egui::Key::N, Action::NewWindow),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::N,
                Action::DuplicateWindow,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::T,
                Action::Reopen,
            ),
            (egui::Modifiers::COMMAND, egui::Key::S, Action::SaveAs),
            (egui::Modifiers::COMMAND, egui::Key::F, Action::Find),
            (
                egui::Modifiers::COMMAND,
                egui::Key::G,
                if cfg!(target_os = "macos") {
                    Action::FindNext
                } else {
                    Action::GoTo
                },
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                egui::Key::G,
                Action::GoTo,
            ),
            (egui::Modifiers::COMMAND, egui::Key::K, Action::Palette),
            (
                egui::Modifiers::COMMAND,
                egui::Key::D,
                if cfg!(target_os = "macos") {
                    Action::ToggleFavorite
                } else {
                    Action::Properties
                },
            ),
            (egui::Modifiers::COMMAND, egui::Key::I, Action::Properties),
            (egui::Modifiers::COMMAND, egui::Key::Y, Action::CustomZoom),
            (egui::Modifiers::COMMAND, egui::Key::Num6, Action::Single),
            (egui::Modifiers::COMMAND, egui::Key::Num7, Action::Facing),
            (egui::Modifiers::COMMAND, egui::Key::Num8, Action::Book),
            (egui::Modifiers::COMMAND, egui::Key::L, Action::Presentation),
            (egui::Modifiers::CTRL, egui::Key::Tab, Action::NextTab),
            (
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                egui::Key::Tab,
                Action::PreviousTab,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::PageDown,
                Action::NextTab,
            ),
            (
                egui::Modifiers::COMMAND,
                egui::Key::PageUp,
                Action::PreviousTab,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::PageDown,
                Action::MoveTabRight,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::PageUp,
                Action::MoveTabLeft,
            ),
            (egui::Modifiers::NONE, egui::Key::F3, Action::FindNext),
            (egui::Modifiers::SHIFT, egui::Key::F3, Action::FindPrevious),
            (egui::Modifiers::NONE, egui::Key::F11, Action::Fullscreen),
            (egui::Modifiers::NONE, egui::Key::F5, Action::Presentation),
            (egui::Modifiers::NONE, egui::Key::F6, Action::Invert),
            (egui::Modifiers::NONE, egui::Key::F8, Action::Toolbar),
            (egui::Modifiers::NONE, egui::Key::F9, Action::MenuBar),
            (egui::Modifiers::NONE, egui::Key::F12, Action::ToggleSidebar),
        ];
        for (modifiers, key, action) in bindings {
            if consume_reader_key(ctx, modifiers, key) {
                self.execute(action, ctx);
            }
        }
        self.update_arrow_scroll(ctx, keyboard);
        if keyboard {
            return;
        }
        let bindings = [
            (
                egui::Modifiers::NONE,
                egui::Key::ArrowLeft,
                Action::ScrollLeft,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::ArrowRight,
                Action::ScrollRight,
            ),
            (egui::Modifiers::NONE, egui::Key::ArrowUp, Action::ScrollUp),
            (
                egui::Modifiers::NONE,
                egui::Key::ArrowDown,
                Action::ScrollDown,
            ),
            (egui::Modifiers::NONE, egui::Key::H, Action::ScrollLeft),
            (egui::Modifiers::NONE, egui::Key::J, Action::ScrollDown),
            (egui::Modifiers::NONE, egui::Key::K, Action::ScrollUp),
            (egui::Modifiers::NONE, egui::Key::L, Action::ScrollRight),
            (egui::Modifiers::NONE, egui::Key::N, Action::Next),
            (egui::Modifiers::NONE, egui::Key::P, Action::Previous),
            (
                egui::Modifiers::SHIFT,
                egui::Key::ArrowUp,
                Action::ScrollUpHalf,
            ),
            (
                egui::Modifiers::SHIFT,
                egui::Key::ArrowDown,
                Action::ScrollDownHalf,
            ),
            (
                egui::Modifiers::SHIFT,
                egui::Key::ArrowLeft,
                Action::ScrollLeftPage,
            ),
            (
                egui::Modifiers::SHIFT,
                egui::Key::ArrowRight,
                Action::ScrollRightPage,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::PageUp,
                Action::ScrollUpPage,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::PageDown,
                Action::ScrollDownPage,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::Space,
                Action::ScrollDownPage,
            ),
            (
                egui::Modifiers::SHIFT,
                egui::Key::Space,
                Action::ScrollUpPage,
            ),
            (
                egui::Modifiers::NONE,
                egui::Key::Enter,
                Action::ScrollDownPage,
            ),
            (
                egui::Modifiers::SHIFT,
                egui::Key::Enter,
                Action::ScrollUpPage,
            ),
            (egui::Modifiers::NONE, egui::Key::Home, Action::First),
            (egui::Modifiers::NONE, egui::Key::End, Action::Last),
            (egui::Modifiers::ALT, egui::Key::ArrowLeft, Action::Back),
            (egui::Modifiers::ALT, egui::Key::ArrowRight, Action::Forward),
            (egui::Modifiers::COMMAND, egui::Key::Plus, Action::ZoomIn),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Plus,
                Action::ZoomIn,
            ),
            (egui::Modifiers::COMMAND, egui::Key::Equals, Action::ZoomIn),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Equals,
                Action::ZoomIn,
            ),
            (egui::Modifiers::COMMAND, egui::Key::Minus, Action::ZoomOut),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num0,
                if cfg!(target_os = "macos") {
                    Action::ActualSize
                } else {
                    Action::FitPage
                },
            ),
            (egui::Modifiers::COMMAND, egui::Key::Num9, Action::FitPage),
            (
                egui::Modifiers::COMMAND,
                egui::Key::Num1,
                Action::ActualSize,
            ),
            (egui::Modifiers::COMMAND, egui::Key::Num2, Action::FitWidth),
            (egui::Modifiers::COMMAND, egui::Key::C, Action::Copy),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::L,
                Action::Highlight,
            ),
            (
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::U,
                Action::Underline,
            ),
            (egui::Modifiers::COMMAND, egui::Key::Z, Action::UndoMarkup),
            (egui::Modifiers::COMMAND, egui::Key::A, Action::SelectAll),
            (
                egui::Modifiers::COMMAND,
                egui::Key::B,
                Action::ToggleFavorite,
            ),
            (egui::Modifiers::CTRL, egui::Key::Tab, Action::NextTab),
            (
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                egui::Key::Tab,
                Action::PreviousTab,
            ),
            (egui::Modifiers::NONE, egui::Key::R, Action::Reload),
            (egui::Modifiers::NONE, egui::Key::G, Action::GoTo),
            (egui::Modifiers::NONE, egui::Key::Q, Action::Close),
            (egui::Modifiers::SHIFT, egui::Key::I, Action::Invert),
            (egui::Modifiers::NONE, egui::Key::C, Action::Continuous),
        ];
        for (modifiers, key, action) in bindings {
            if consume_reader_key(ctx, modifiers, key) {
                self.execute(action, ctx);
            }
        }
    }
}
