use super::{Action, ReaderApp, icons, theme};
use eframe::egui::{self, Align, Layout, Rect, Sense, Vec2};

impl ReaderApp {
    pub(super) fn fullscreen_reading_controls(&mut self, ctx: &egui::Context) {
        let screen = ctx.viewport_rect();
        let visible = ctx.input(|input| {
            input.pointer.hover_pos().is_some_and(|pointer| {
                pointer.y <= screen.top() + 8.0
                    || self
                        .fullscreen_controls
                        .is_some_and(|rect| rect.contains(pointer))
            })
        });
        if !visible {
            self.fullscreen_controls = None;
            return;
        }
        let mut action = None;
        let response = egui::Area::new(egui::Id::new("fullscreen-reading-controls"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 4.0))
            .movable(false)
            .show(ctx, |ui| {
                ui.set_max_width((screen.width() - 16.0).min(800.0));
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                        ui.horizontal_wrapped(|ui| {
                            if icons::button(
                                ui,
                                icons::Icon::Previous,
                                "Página anterior",
                                self.enabled(Action::Previous),
                                false,
                            )
                            .clicked()
                            {
                                action = Some(Action::Previous);
                            }
                            if let Some(document) = &self.state().document
                                && ui
                                    .button(format!(
                                        "{} / {}",
                                        document.page + 1,
                                        document.info.page_count
                                    ))
                                    .clicked()
                            {
                                action = Some(Action::GoTo);
                            }
                            if icons::button(
                                ui,
                                icons::Icon::Next,
                                "Página siguiente",
                                self.enabled(Action::Next),
                                false,
                            )
                            .clicked()
                            {
                                action = Some(Action::Next);
                            }
                            for (label, choice) in [
                                ("Ajustar ancho", Action::FitWidth),
                                ("Buscar", Action::Find),
                                ("Panel", Action::ToggleSidebar),
                                ("Comandos", Action::Palette),
                                ("Salir · F11", Action::Fullscreen),
                            ] {
                                if ui.button(label).clicked() {
                                    action = Some(choice);
                                }
                            }
                        });
                    });
            });
        self.fullscreen_controls = Some(response.response.rect);
        if let Some(action) = action {
            self.execute(action, ctx);
        }
    }

    pub(super) fn titlebar(&mut self, ui: &mut egui::Ui) {
        let custom = !self.preferences.native_titlebar && !cfg!(target_os = "macos");
        let controls = if self.fullscreen {
            108.0
        } else if custom {
            138.0
        } else {
            0.0
        };
        let mut activate = None;
        let mut close = None;
        let mut action = None;
        let mut started_drag = None;
        let bar = egui::Panel::top("window-titlebar")
            .exact_size(36.0)
            .frame(
                egui::Frame::NONE
                    .fill(theme::palette(self.preferences.theme).surface)
                    .inner_margin(egui::Margin::symmetric(6, 3)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().button_padding = egui::vec2(7.0, 3.0);
                ui.spacing_mut().interact_size.y = 24.0;
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                ui.horizontal(|ui| {
                    if cfg!(target_os = "macos") && !self.fullscreen {
                        ui.add_space(76.0);
                    }
                    if ui
                        .add(egui::Button::new("Inicio").selected(self.home))
                        .clicked()
                    {
                        action = Some(Action::Home);
                    }
                    if ui
                        .add_enabled(self.enabled(Action::Open), egui::Button::new("+"))
                        .on_hover_text("Abrir documento · Ctrl/⌘ + O")
                        .clicked()
                    {
                        action = Some(Action::Open);
                    }
                    let available = (ui.available_width() - controls - 50.0).max(40.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(available, 28.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            egui::ScrollArea::horizontal()
                                .id_salt("titlebar-tabs")
                                .scroll_bar_visibility(
                                    egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                )
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        for (index, tab) in self.tabs.iter().enumerate() {
                                            ui.push_id(tab.reader.id, |ui| {
                                            let selected = !self.home && index == self.active;
                                            egui::Frame::new()
                                                .inner_margin(egui::Margin::symmetric(2, 1))
                                                .show(ui, |ui| {
                                                    ui.horizontal(|ui| {
                                                        let name = tab.reader.path
                                                            .file_name()
                                                            .unwrap_or_default()
                                                            .to_string_lossy();
                                                        let label = if tab.reader.state.opening.is_some() {
                                                            format!("{name} …")
                                                        } else {
                                                            name.into_owned()
                                                        };
                                                        let button = ui
                                                            .add_sized(
                                                                [160.0, 22.0],
                                                                egui::Button::new(label)
                                                                    .truncate()
                                                                    .selected(selected)
                                                                    .sense(Sense::click_and_drag()),
                                                            )
                                                            .on_hover_text(format!(
                                                                "{}\nArrastra fuera de la barra para duplicar la pestaña.",
                                                                tab.reader.path.display(),
                                                            ))
                                                            .on_hover_cursor(egui::CursorIcon::Grab);
                                                        self.tab_buttons.push((tab.reader.id, button.rect));
                                                        if button.drag_started_by(egui::PointerButton::Primary) {
                                                            started_drag = Some(tab.reader.id);
                                                        }
                                                        if button.clicked() {
                                                            activate = Some(index);
                                                        }
                                                        if selected && self.reveal_tab {
                                                            button
                                                                .scroll_to_me(Some(Align::Center));
                                                        }
                                                        button.context_menu(|ui| {
                                                            for choice in [
                                                                Action::DuplicateTab,
                                                                Action::DuplicateWindow,
                                                                Action::Close,
                                                                Action::CloseOthers,
                                                                Action::CloseLeft,
                                                                Action::CloseRight,
                                                                Action::MoveTabLeft,
                                                                Action::MoveTabRight,
                                                            ] {
                                                                if ui
                                                                    .button(choice.label())
                                                                    .clicked()
                                                                {
                                                                    activate = Some(index);
                                                                    action = Some(choice);
                                                                    ui.close();
                                                                }
                                                            }
                                                        });
                                                        if icons::close_button(
                                                            ui,
                                                            egui::vec2(24.0, 24.0),
                                                            false,
                                                            "Cerrar pestaña · Ctrl/⌘ + W",
                                                        )
                                                        .clicked()
                                                        {
                                                            close = Some(index);
                                                        }
                                                    });
                                                });
                                            });
                                        }
                                    });
                                });
                        },
                    );
                    let drag_width = (ui.available_width() - controls).max(20.0);
                    let (_, drag) = ui
                        .allocate_exact_size(egui::vec2(drag_width, 26.0), Sense::click_and_drag());
                    if drag.double_clicked() {
                        action = Some(Action::Maximize);
                    }
                    if drag.drag_started_by(egui::PointerButton::Primary) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if self.fullscreen {
                        if ui.button("Salir · F11").clicked() {
                            action = Some(Action::Fullscreen);
                        }
                    } else if custom {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        if icons::caption_button(
                            ui,
                            icons::Caption::Minimize,
                            egui::vec2(44.0, 30.0),
                            true,
                            "Minimizar",
                        )
                        .clicked()
                        {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        let maximized =
                            ui.input(|input| input.viewport().maximized.unwrap_or(false));
                        if icons::caption_button(
                            ui,
                            icons::Caption::Maximize(maximized),
                            egui::vec2(44.0, 30.0),
                            true,
                            "Maximizar / restaurar",
                        )
                        .clicked()
                        {
                            action = Some(Action::Maximize);
                        }
                        if icons::close_button(
                            ui,
                            egui::vec2(44.0, 30.0),
                            true,
                            "Cerrar ventana · Alt + F4",
                        )
                        .clicked()
                        {
                            action = Some(Action::Exit);
                        }
                    }
                });
            });
        self.reveal_tab = false;
        if let Some(id) = started_drag {
            self.tab_drag = Some(id);
        }
        if ui.input(|i| {
            !i.pointer.primary_down() && !i.pointer.button_released(egui::PointerButton::Primary)
        }) {
            self.tab_drag = None;
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let outside = pointer.is_some_and(|pos| !bar.response.rect.expand(12.0).contains(pos));
        if self.tab_drag.is_some() && ui.input(|i| i.pointer.primary_down()) {
            ui.ctx().set_cursor_icon(if outside {
                egui::CursorIcon::Copy
            } else {
                egui::CursorIcon::Grabbing
            });
            if outside && let Some(pos) = pointer {
                egui::Area::new(egui::Id::new("duplicate-tab-drag-hint"))
                    .order(egui::Order::Tooltip)
                    .interactable(false)
                    .fixed_pos(pos + egui::vec2(12.0, 18.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label("Soltar para abrir en otra ventana");
                        });
                    });
            }
        }
        if ui.input(|i| i.pointer.button_released(egui::PointerButton::Primary))
            && let Some(id) = self.tab_drag.take()
            && outside
            && let Some(index) = self.tabs.iter().position(|tab| tab.reader.id == id)
            && self.tabs[index].reader.state.document.is_some()
        {
            activate = Some(index);
            action = Some(Action::DuplicateWindow);
        }
        if let Some(index) = activate {
            self.active = index;
            self.home = false;
            self.reveal_tab = true;
        }
        if let Some(index) = close {
            self.close_index(index);
            self.reveal_tab = true;
        }
        if let Some(action) = action {
            self.execute(action, ui.ctx());
        }
    }

    pub(super) fn resize_edges(&self, ctx: &egui::Context) {
        if self.fullscreen
            || self.presentation
            || self.preferences.native_titlebar
            || cfg!(target_os = "macos")
            || ctx.input(|i| i.viewport().maximized.unwrap_or(false))
        {
            return;
        }
        let rect = ctx.viewport_rect();
        let width = rect.width();
        let height = rect.height();
        let corner = 9.0;
        let edge = 4.0;
        use egui::ResizeDirection as D;
        let zones = [
            (
                D::NorthWest,
                Rect::from_min_size(rect.min, Vec2::splat(corner)),
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                D::NorthEast,
                Rect::from_min_size(egui::pos2(width - corner, 0.0), Vec2::splat(corner)),
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                D::SouthWest,
                Rect::from_min_size(egui::pos2(0.0, height - corner), Vec2::splat(corner)),
                egui::CursorIcon::ResizeNeSw,
            ),
            (
                D::SouthEast,
                Rect::from_min_size(
                    egui::pos2(width - corner, height - corner),
                    Vec2::splat(corner),
                ),
                egui::CursorIcon::ResizeNwSe,
            ),
            (
                D::North,
                Rect::from_min_size(
                    egui::pos2(corner, 0.0),
                    egui::vec2(width - 2.0 * corner, edge),
                ),
                egui::CursorIcon::ResizeVertical,
            ),
            (
                D::South,
                Rect::from_min_size(
                    egui::pos2(corner, height - edge),
                    egui::vec2(width - 2.0 * corner, edge),
                ),
                egui::CursorIcon::ResizeVertical,
            ),
            (
                D::West,
                Rect::from_min_size(
                    egui::pos2(0.0, corner),
                    egui::vec2(edge, height - 2.0 * corner),
                ),
                egui::CursorIcon::ResizeHorizontal,
            ),
            (
                D::East,
                Rect::from_min_size(
                    egui::pos2(width - edge, corner),
                    egui::vec2(edge, height - 2.0 * corner),
                ),
                egui::CursorIcon::ResizeHorizontal,
            ),
        ];
        for (index, (direction, rect, cursor)) in zones.into_iter().enumerate() {
            egui::Area::new(egui::Id::new(("window-resize", index)))
                .order(egui::Order::Foreground)
                .movable(false)
                .fixed_pos(rect.min)
                .show(ctx, |ui| {
                    let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());
                    let response = response.on_hover_cursor(cursor);
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                    }
                });
        }
    }
}
