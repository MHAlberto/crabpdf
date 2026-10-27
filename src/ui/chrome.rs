use super::{Action, ReaderApp, icons, theme};
use crate::desktop::command_meta::ActionMetadata;
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
        #[cfg(test)]
        self.titlebar_control_rects.clear();
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
                ui.horizontal_centered(|ui| {
                    if cfg!(target_os = "macos") && !self.fullscreen {
                        ui.add_space(76.0);
                    }
                    let home = ui.add(egui::Button::new("Inicio").selected(self.home));
                    #[cfg(test)]
                    self.titlebar_control_rects.push(home.rect);
                    if home.clicked() {
                        action = Some(Action::Home);
                    }
                    let open = ui.add_enabled(self.enabled(Action::Open), egui::Button::new("+"))
                        .on_hover_text("Abrir documento · Ctrl/⌘ + O");
                    #[cfg(test)]
                    self.titlebar_control_rects.push(open.rect);
                    if open.clicked() {
                        action = Some(Action::Open);
                    }
                    // Match the real tab cells instead of giving the scroll area the
                    // unused titlebar. Empty space must belong to native window dragging.
                    const LABEL_WIDTH: f32 = 160.0;
                    const TAB_CLOSE: f32 = 24.0;
                    const TAB_MARGIN: f32 = 4.0;
                    let gap = ui.spacing().item_spacing.x;
                    let natural_width = self.tabs.len() as f32
                        * (LABEL_WIDTH + TAB_CLOSE + TAB_MARGIN + gap)
                        + self.tabs.len().saturating_sub(1) as f32 * gap;
                    let available = (ui.available_width() - controls - 50.0).max(40.0);
                    if !self.tabs.is_empty() {
                        ui.allocate_ui_with_layout(
                            egui::vec2(available.min(natural_width), 24.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                egui::ScrollArea::horizontal()
                                    .id_salt("titlebar-tabs")
                                    .scroll_bar_visibility(
                                        egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                    )
                                    .auto_shrink([false, true])
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            for (index, tab) in self.tabs.iter().enumerate() {
                                                ui.push_id(tab.reader.id, |ui| {
                                                let selected = !self.home && index == self.active;
                                                egui::Frame::new()
                                                    .inner_margin(egui::Margin::symmetric(2, 0))
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
                                                                    [LABEL_WIDTH, 24.0],
                                                                    egui::Button::new(label)
                                                                        .truncate()
                                                                        .selected(selected)
                                                                        .sense(Sense::click_and_drag()),
                                                                )
                                                                .on_hover_ui(|ui| {
                                                                    ui.label(tab.reader.path.display().to_string());
                                                                    ui.label("Arrastra fuera de la barra para abrir otra ventana.");
                                                                })
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
                    }
                    let drag_width = (ui.available_width() - controls).max(20.0);
                    let (_, drag) = ui
                        .allocate_exact_size(egui::vec2(drag_width, 30.0), Sense::click_and_drag());
                    #[cfg(test)]
                    {
                        self.titlebar_drag_rect = Some(drag.rect);
                    }
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
                        let minimize = icons::caption_button(
                            ui,
                            icons::Caption::Minimize,
                            egui::vec2(44.0, 30.0),
                            true,
                            "Minimizar",
                        );
                        #[cfg(test)]
                        self.titlebar_control_rects.push(minimize.rect);
                        if minimize.clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        let maximized =
                            ui.input(|input| input.viewport().maximized.unwrap_or(false));
                        let maximize = icons::caption_button(
                            ui,
                            icons::Caption::Maximize(maximized),
                            egui::vec2(44.0, 30.0),
                            true,
                            "Maximizar / restaurar",
                        );
                        #[cfg(test)]
                        self.titlebar_control_rects.push(maximize.rect);
                        if maximize.clicked() {
                            action = Some(Action::Maximize);
                        }
                        let close_window = icons::close_button(
                            ui,
                            egui::vec2(44.0, 30.0),
                            true,
                            "Cerrar ventana · Alt + F4",
                        );
                        #[cfg(test)]
                        self.titlebar_control_rects.push(close_window.rect);
                        if close_window.clicked() {
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

#[cfg(test)]
mod titlebar_tests {
    use super::*;
    use crate::{document::DocumentInfo, ui::tab_view::Tab};

    fn reader(count: usize) -> (egui::Context, ReaderApp) {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = ReaderApp::with_config(&cc, None, None);
        // No native documents or GPU are needed to exercise the real header hit testing.
        app.worker = None;
        for index in 0..count {
            let id = index as u64 + 1;
            let path = std::path::PathBuf::from(format!("book-{index}.pdf"));
            let mut tab = Tab::new(id, path.clone(), None);
            tab.reader.state.finish_open(
                id,
                Ok(DocumentInfo {
                    path,
                    page_count: 3,
                }),
            );
            app.tabs.push(tab);
        }
        app.home = count == 0;
        (ctx, app)
    }
    fn frame(
        ctx: &egui::Context,
        app: &mut ReaderApp,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        app.tab_buttons.clear();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1080.0, 780.0),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| app.titlebar(ui));
            },
        );
        output.textures_delta.clear();
        output
    }
    fn press(position: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(position),
            egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    fn started_window_drag(output: &egui::FullOutput) -> bool {
        output.viewport_output.values().any(|viewport| {
            viewport
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::StartDrag))
        })
    }

    #[test]
    fn unused_header_space_drags_the_window_and_does_not_detach_a_tab() {
        let (ctx, mut app) = reader(1);
        let _ = frame(&ctx, &mut app, vec![]);
        let _ = frame(&ctx, &mut app, vec![]);
        let drag = app.titlebar_drag_rect.unwrap();
        let position = egui::pos2(700.0, drag.center().y);
        assert!(
            drag.width() > 480.0,
            "Empty header was consumed by the tab scroller: {drag:?}"
        );
        assert!(drag.contains(position));
        let down = frame(&ctx, &mut app, press(position));
        let moved = frame(
            &ctx,
            &mut app,
            vec![egui::Event::PointerMoved(position + egui::vec2(24.0, 0.0))],
        );
        assert!(started_window_drag(&down) || started_window_drag(&moved));
        assert!(
            app.tab_drag.is_none(),
            "Window dragging was mistaken for tab detachment"
        );
    }

    #[test]
    fn tabs_keep_their_own_drag_and_controls_share_a_center_line() {
        for (count, density) in [(1, 1.0), (8, 1.0), (1, 1.25), (8, 1.5)] {
            let (ctx, mut app) = reader(count);
            ctx.set_pixels_per_point(density);
            let _ = frame(&ctx, &mut app, vec![]);
            let _ = frame(&ctx, &mut app, vec![]);
            let tab = app.tab_buttons[0].1;
            for rect in &app.titlebar_control_rects {
                assert!(
                    (rect.center().y - tab.center().y).abs() < 0.75,
                    "Misaligned controls: {rect:?} versus {tab:?}"
                );
            }
            assert!(!app.titlebar_drag_rect.unwrap().intersects(tab));
            let down = frame(&ctx, &mut app, press(tab.center()));
            let moved = frame(
                &ctx,
                &mut app,
                vec![egui::Event::PointerMoved(
                    tab.center() + egui::vec2(20.0, 0.0),
                )],
            );
            assert_eq!(app.tab_drag, Some(1));
            assert!(!started_window_drag(&down) && !started_window_drag(&moved));
        }
    }

    #[test]
    fn native_title_tracks_document_and_home_after_cached_frames() {
        let (ctx, mut app) = reader(2);
        let mut tick = || {
            let mut frame = eframe::Frame::_new_kittest();
            let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                eframe::App::logic(&mut app, ctx.ctx(), &mut frame);
            });
            output.textures_delta.clear();
            output
                .viewport_output
                .values()
                .flat_map(|viewport| &viewport.commands)
                .filter_map(|command| match command {
                    egui::ViewportCommand::Title(title) => Some(title.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(tick(), ["book-0.pdf · CrabPDF"]);
        assert!(tick().is_empty());
        app.active = 1;
        let mut frame = eframe::Frame::_new_kittest();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut app, ctx.ctx(), &mut frame)
        });
        output.textures_delta.clear();
        assert!(output.viewport_output.values().flat_map(|viewport| &viewport.commands)
            .any(|command| matches!(command, egui::ViewportCommand::Title(title) if title == "book-1.pdf · CrabPDF")));
        app.home = true;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            eframe::App::logic(&mut app, ctx.ctx(), &mut frame)
        });
        output.textures_delta.clear();
        assert!(output.viewport_output.values().flat_map(|viewport| &viewport.commands)
            .any(|command| matches!(command, egui::ViewportCommand::Title(title) if title == "CrabPDF")));
    }
}
