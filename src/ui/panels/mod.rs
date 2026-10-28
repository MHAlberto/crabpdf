//! Menus, toolbar, search and transient notices.
mod dialogs;
mod sidebar;

use super::{Action, ReaderApp, icons, theme};
use crate::desktop::command_meta::ActionMetadata;
use crate::ui::icons::Icon;
use crabpdf_core::document::{ErrorKind, Zoom};
use crabpdf_core::layout::LayoutMode;
use eframe::egui::{self, Align, Layout, RichText};

impl ReaderApp {
    pub(super) fn markup_colors(&mut self, ui: &mut egui::Ui) {
        let colors = [
            ("Amarillo", [255, 230, 50]),
            ("Verde", [90, 210, 100]),
            ("Azul", [80, 170, 255]),
            ("Rosa", [255, 120, 190]),
            ("Naranja", [255, 165, 60]),
            ("Rojo", [245, 85, 85]),
        ];
        let selected_name = colors
            .iter()
            .find(|(_, rgb)| *rgb == self.preferences.markup_color)
            .map_or("Personalizado", |(name, _)| *name);
        ui.label(RichText::new(format!("Color · {selected_name}")).small());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            for (label, rgb) in colors {
                let selected = self.preferences.markup_color == rgb;
                let response = ui.add_sized([30.0, 30.0], egui::Button::new("").frame(false));
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label)
                });
                let center = response.rect.center();
                let color = egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                ui.painter().circle_filled(center, 10.0, color);
                if selected || response.hovered() || response.has_focus() {
                    ui.painter().circle_stroke(
                        center,
                        13.0,
                        egui::Stroke::new(
                            if selected { 2.0 } else { 1.0 },
                            ui.visuals().selection.stroke.color,
                        ),
                    );
                }
                if selected {
                    let ink = egui::Color32::from_rgb(28, 31, 36);
                    let stroke = egui::Stroke::new(1.7, ink);
                    ui.painter().line_segment(
                        [
                            center + egui::vec2(-4.0, 0.0),
                            center + egui::vec2(-1.0, 3.0),
                        ],
                        stroke,
                    );
                    ui.painter().line_segment(
                        [
                            center + egui::vec2(-1.0, 3.0),
                            center + egui::vec2(5.0, -4.0),
                        ],
                        stroke,
                    );
                }
                if response.on_hover_text(label).clicked() {
                    self.set_markup_color(rgb);
                    ui.close();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Personalizado");
            let mut color = self.preferences.markup_color;
            if ui.color_edit_button_srgb(&mut color).changed() {
                self.set_markup_color(color);
            }
            let [r, g, b] = self.preferences.markup_color;
            ui.label(
                RichText::new(format!("#{r:02X}{g:02X}{b:02X}"))
                    .monospace()
                    .small(),
            );
        });
    }
    pub(super) fn menu_action(&mut self, ui: &mut egui::Ui, action: Action) {
        if ui
            .add_enabled(
                self.enabled(action),
                egui::Button::new(action.label()).shortcut_text(
                    action
                        .shortcut()
                        .unwrap_or("")
                        .replace(
                            "Ctrl/⌘",
                            if cfg!(target_os = "macos") {
                                "⌘"
                            } else {
                                "Ctrl"
                            },
                        )
                        .replace(" + ", "+"),
                ),
            )
            .clicked()
        {
            self.execute(action, ui.ctx());
            ui.close();
        }
    }
    pub(super) fn menu(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("menu").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Archivo", |ui| {
                    for action in [
                        Action::Open,
                        Action::Home,
                        Action::SaveAs,
                        Action::Reload,
                        Action::NewWindow,
                        Action::DuplicateWindow,
                        Action::DuplicateTab,
                        Action::Reveal,
                        Action::CopyPath,
                        Action::Properties,
                    ] {
                        self.menu_action(ui, action);
                    }
                    ui.separator();
                    ui.menu_button("Recientes", |ui| {
                        for saved in self.preferences.recent.clone() {
                            if ui
                                .button(
                                    saved.path.file_name().unwrap_or_default().to_string_lossy(),
                                )
                                .on_hover_text(saved.path.display().to_string())
                                .clicked()
                            {
                                self.open_document(saved.path.clone(), Some(saved));
                                ui.close();
                            }
                        }
                        if self.preferences.recent.is_empty() {
                            ui.label("Sin documentos recientes");
                        }
                        ui.separator();
                        self.menu_action(ui, Action::ClearHistory);
                    });
                    ui.separator();
                    for action in [
                        Action::Close,
                        Action::CloseOthers,
                        Action::CloseLeft,
                        Action::CloseRight,
                        Action::CloseAll,
                        Action::Reopen,
                        Action::Exit,
                    ] {
                        self.menu_action(ui, action);
                    }
                });
                ui.menu_button("Ver", |ui| {
                    for action in [
                        Action::Single,
                        Action::Facing,
                        Action::Book,
                        Action::Continuous,
                        Action::VerticalReading,
                        Action::Manga,
                    ] {
                        self.menu_action(ui, action);
                    }
                    ui.separator();
                    for action in [
                        Action::RotateLeft,
                        Action::RotateRight,
                        Action::Fullscreen,
                        Action::Maximize,
                        Action::Presentation,
                        Action::Toolbar,
                    ] {
                        self.menu_action(ui, action);
                    }
                    ui.separator();
                    for action in [
                        Action::Theme,
                        Action::Invert,
                        Action::Grayscale,
                        Action::Settings,
                    ] {
                        self.menu_action(ui, action);
                    }
                });
                ui.menu_button("Ir", |ui| {
                    for action in [
                        Action::Previous,
                        Action::Next,
                        Action::First,
                        Action::Last,
                        Action::GoTo,
                        Action::Back,
                        Action::Forward,
                    ] {
                        self.menu_action(ui, action);
                    }
                });
                ui.menu_button("Zoom", |ui| {
                    for action in [
                        Action::ZoomIn,
                        Action::ZoomOut,
                        Action::ActualSize,
                        Action::FitWidth,
                        Action::FitHeight,
                        Action::FitPage,
                        Action::CustomZoom,
                    ] {
                        self.menu_action(ui, action);
                    }
                });
                ui.menu_button("Texto", |ui| {
                    for action in [
                        Action::Find,
                        Action::FindNext,
                        Action::FindPrevious,
                        Action::SelectAll,
                        Action::Copy,
                        Action::Highlight,
                        Action::Underline,
                        Action::UndoMarkup,
                    ] {
                        self.menu_action(ui, action);
                    }
                    ui.separator();
                    self.markup_colors(ui);
                });
                ui.menu_button("Paneles", |ui| {
                    for action in [
                        Action::Thumbnails,
                        Action::Outline,
                        Action::Favorites,
                        Action::ToggleFavorite,
                        Action::Palette,
                    ] {
                        self.menu_action(ui, action);
                    }
                });
            });
        });
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(self.preferences.theme);
        egui::Panel::top("toolbar")
            .frame(
                egui::Frame::NONE
                    .fill(palette.surface)
                    .inner_margin(egui::Margin::symmetric(12, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let buttons = [
                        (Icon::Sidebar, Action::ToggleSidebar),
                        (Icon::Open, Action::Open),
                        (Icon::Previous, Action::Previous),
                    ];
                    for (icon, action) in buttons {
                        if icons::button(ui, icon, action.label(), self.enabled(action), false)
                            .clicked()
                        {
                            self.execute(action, ui.ctx());
                        }
                    }
                    let (page, count) = self
                        .state()
                        .document
                        .as_ref()
                        .map_or((0, 0), |d| (d.page + 1, d.info.page_count));
                    if ui
                        .add_enabled(
                            count > 0,
                            egui::Button::new(
                                RichText::new(format!("{page} / {count}")).monospace(),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Ir a página · Ctrl/⌘ + G")
                        .clicked()
                    {
                        self.execute(Action::GoTo, ui.ctx());
                    }
                    if icons::button(
                        ui,
                        Icon::Next,
                        Action::Next.label(),
                        self.enabled(Action::Next),
                        false,
                    )
                    .clicked()
                    {
                        self.execute(Action::Next, ui.ctx());
                    }
                    ui.separator();
                    if icons::button(ui, Icon::Minus, Action::ZoomOut.label(), count > 0, false)
                        .clicked()
                    {
                        self.execute(Action::ZoomOut, ui.ctx());
                    }
                    let zoom = self.effective_zoom();
                    if ui
                        .add_enabled(
                            count > 0,
                            egui::Button::new(
                                RichText::new(if count > 0 {
                                    format!("{zoom:.0}%")
                                } else {
                                    "—".into()
                                })
                                .monospace(),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Zoom personalizado")
                        .clicked()
                    {
                        self.execute(Action::CustomZoom, ui.ctx());
                    }
                    if icons::button(ui, Icon::Plus, Action::ZoomIn.label(), count > 0, false)
                        .clicked()
                    {
                        self.execute(Action::ZoomIn, ui.ctx());
                    }
                    if icons::button(
                        ui,
                        Icon::FitWidth,
                        Action::FitWidth.label(),
                        count > 0,
                        self.state().zoom == Zoom::FitWidth && count > 0,
                    )
                    .clicked()
                    {
                        self.execute(Action::FitWidth, ui.ctx());
                    }
                    if ui
                        .add_enabled(count > 0, egui::Button::new("Buscar"))
                        .on_hover_text("Buscar texto · Ctrl/⌘ + F")
                        .clicked()
                    {
                        self.execute(Action::Find, ui.ctx());
                    }
                    if ui
                        .add_enabled(
                            count > 0,
                            egui::Button::new("Vertical").selected(
                                self.state().continuous
                                    && self.state().layout == LayoutMode::Single
                                    && self.state().zoom == Zoom::FitWidth,
                            ),
                        )
                        .on_hover_text(Action::VerticalReading.label())
                        .clicked()
                    {
                        self.execute(Action::VerticalReading, ui.ctx());
                    }
                    let marks = ui
                        .add_enabled_ui(count > 0, |ui| {
                            ui.menu_button("Marcas", |ui| {
                                ui.set_min_width(236.0);
                                ui.set_max_width(280.0);
                                ui.spacing_mut().item_spacing.y = 6.0;
                                ui.strong("Marcas de texto");
                                if !self.enabled(Action::Highlight) {
                                    ui.add(egui::Label::new(RichText::new(
                                "Selecciona texto del documento para resaltar o subrayar."
                            ).small().color(palette.muted)).wrap());
                                }
                                for action in [Action::Highlight, Action::Underline] {
                                    self.menu_action(ui, action);
                                }
                                ui.separator();
                                self.markup_colors(ui);
                                ui.separator();
                                for action in [Action::UndoMarkup, Action::SaveAs] {
                                    self.menu_action(ui, action);
                                }
                            })
                        })
                        .inner;
                    self.markup_button = Some(marks.response.rect);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if icons::button(ui, Icon::Theme, Action::Theme.label(), true, false)
                            .clicked()
                        {
                            self.execute(Action::Theme, ui.ctx());
                        }
                        if ui.available_width() > 70.0 {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(crate::branding::NAME).color(palette.muted),
                                )
                                .truncate(),
                            )
                            .on_hover_text({
                                let limited = self.state().requested.is_some_and(|key| {
                                    self.pages.get(&key).is_some_and(|page| page.limited)
                                });
                                format!(
                                    "{} {}{}",
                                    crate::branding::NAME,
                                    env!("CARGO_PKG_VERSION"),
                                    if limited {
                                        "\nResolución limitada para controlar la memoria."
                                    } else {
                                        ""
                                    }
                                )
                            });
                        }
                    });
                });
            });
    }

    pub(super) fn search_bar(&mut self, ui: &mut egui::Ui) {
        if self.home {
            return;
        }
        let Some(tab) = self.tabs.get(self.active) else {
            return;
        };
        let was_open = tab.view.search.open;
        let mut open = was_open;
        let mut changed = false;
        let mut action = None;
        egui::Panel::top("find").resizable(false).show_collapsible(ui, &mut open, |ui| {
                if !was_open { ui.disable(); }
            ui.horizontal_wrapped(|ui| {
                let tab = &mut self.tabs[self.active];
                ui.label("Buscar:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut tab.reader.search.query)
                        .desired_width(240.0)
                        .hint_text("Texto del documento"),
                );
                if tab.view.search.focus {
                    response.request_focus();
                    tab.view.search.focus = false;
                }
                changed |= response.changed();
                changed |= ui
                    .checkbox(&mut tab.reader.search.match_case, "Aa")
                    .on_hover_text("Distinguir mayúsculas")
                    .changed();
                changed |= ui
                    .checkbox(&mut tab.reader.search.whole_word, "Palabra completa")
                    .changed();
                let count = tab.reader.search.hits.len();
                ui.label(format!(
                    "{} / {count}",
                    if count == 0 { 0 } else { tab.reader.search.index + 1 }
                ));
                if ui
                    .add_enabled(count > 0, egui::Button::new("Anterior"))
                    .clicked()
                {
                    action = Some(Action::FindPrevious);
                }
                if ui
                    .add_enabled(count > 0, egui::Button::new("Siguiente"))
                    .clicked()
                {
                    action = Some(Action::FindNext);
                }
                if tab.reader.search.limited { ui.label("Límite de 10 000 coincidencias").on_hover_text("Refina la búsqueda para consultar otros resultados. El límite controla la memoria de documentos extensos."); }
                if tab.reader.search.is_scanning() {
                    ui.spinner();
                    ui.label(format!(
                        "{}/{} páginas",
                        tab.reader.search.scanned, tab.reader.search.total
                    ));
                }
                if icons::close_button(ui, egui::vec2(24.0, 24.0), false, "Cerrar búsqueda · Escape").clicked() {
                    tab.view.search.open = false;
                }
                if response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    action = Some(if ui.input(|i| i.modifiers.shift) {
                        Action::FindPrevious
                    } else {
                        Action::FindNext
                    });
                }
            });
        });
        if changed {
            self.start_search();
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(200));
        }
        if let Some(action) = action {
            self.execute(action, ui.ctx());
        }
    }

    pub(super) fn notices(&mut self, ui: &mut egui::Ui) {
        let document_error = self.state().error.clone();
        let search_error = document_error.is_none();
        if let Some(error) = document_error.or_else(|| self.search_error().cloned()) {
            egui::Panel::top("error")
                .frame(
                    egui::Frame::NONE
                        .inner_margin(10)
                        .fill(ui.visuals().extreme_bg_color),
                )
                .show(ui, |ui| {
                    ui.label(RichText::new(&error.message).color(ui.visuals().error_fg_color));
                    if let Some(details) = &error.details {
                        egui::CollapsingHeader::new("Detalles técnicos").show(ui, |ui| {
                            ui.add(
                                egui::Label::new(RichText::new(details).monospace().small()).wrap(),
                            );
                        });
                    }
                    ui.horizontal(|ui| {
                        if !search_error
                            && self.password_retry_available()
                            && ui.button("Introducir contraseña").clicked()
                            && let Some(tab) = self.tabs.get_mut(self.active)
                        {
                            tab.reader.password_needed = true;
                        }
                        if search_error && ui.button("Reintentar búsqueda").clicked() {
                            self.start_search();
                            if let Some(tab) = self.tabs.get_mut(self.active) {
                                tab.view.search.open = true;
                            }
                        }
                        if !search_error
                            && error.kind == ErrorKind::Render
                            && ui.button("Reintentar").clicked()
                            && let Some(tab) = self.tabs.get_mut(self.active)
                        {
                            tab.reader.state.invalidate();
                            self.pending_pages.clear();
                        }
                        if self.worker.is_some() && ui.small_button("Cerrar aviso").clicked() {
                            if let Some(tab) = self.tabs.get_mut(self.active) {
                                if search_error {
                                    tab.reader.search.dismiss_error();
                                } else {
                                    tab.reader.state.error = None;
                                }
                            } else {
                                self.empty.error = None;
                            }
                        }
                    });
                });
        }
        if let Some(warning) = self.config_warning.clone() {
            egui::Panel::top("configuration-warning").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(warning);
                    if ui.small_button("Cerrar").clicked() {
                        self.config_warning = None;
                    }
                });
            });
        }
        if let Some(notice) = self.notice.clone() {
            if self
                .notice_started
                .as_ref()
                .is_none_or(|(message, _)| message != &notice)
            {
                self.notice_started = Some((notice.clone(), std::time::Instant::now()));
            }
            let elapsed = self
                .notice_started
                .as_ref()
                .map_or(std::time::Duration::ZERO, |(_, started)| started.elapsed());
            if elapsed >= std::time::Duration::from_secs(4) {
                self.notice = None;
                self.notice_started = None;
                return;
            }
            let mut dismiss = false;
            egui::Area::new(egui::Id::new("reader-toast"))
                .order(egui::Order::Foreground)
                .anchor(
                    egui::Align2::RIGHT_TOP,
                    egui::vec2(-10.0, ui.available_rect_before_wrap().top() + 8.0),
                )
                .movable(false)
                .fade_in(!self.preferences.reduce_motion)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .inner_margin(8)
                        .show(ui, |ui| {
                            ui.set_max_width(320.0);
                            ui.horizontal(|ui| {
                                ui.add(egui::Label::new(RichText::new(notice).small()).wrap());
                                dismiss = icons::close_button(
                                    ui,
                                    egui::vec2(24.0, 24.0),
                                    false,
                                    "Cerrar aviso",
                                )
                                .clicked();
                            });
                        });
                });
            if dismiss {
                self.notice = None;
                self.notice_started = None;
            } else {
                ui.ctx().request_repaint_after(
                    std::time::Duration::from_secs(4).saturating_sub(elapsed),
                );
            }
        } else {
            self.notice_started = None;
        }
    }
}
