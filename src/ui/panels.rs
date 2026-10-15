use super::{Action, ReaderApp, Sidebar, icons, theme};
use crate::config::StoredDocument;
use crate::document::{ErrorKind, PageColors, Zoom};
use crate::layout::LayoutMode;
use eframe::egui::{self, Align, Layout, RichText};
use icons::Icon;

impl ReaderApp {
    pub(super) fn markup_colors(&mut self, ui: &mut egui::Ui) {
        ui.label("Color de las marcas");
        for (label, color) in [
            ("Amarillo", [255, 230, 50]),
            ("Verde", [90, 210, 100]),
            ("Azul", [80, 170, 255]),
            ("Rosa", [255, 120, 190]),
            ("Naranja", [255, 165, 60]),
            ("Rojo", [245, 85, 85]),
        ] {
            let selected = self.preferences.markup_color == color;
            let text = RichText::new(if selected {
                format!("✓ {label}")
            } else {
                label.into()
            })
            .color(egui::Color32::BLACK);
            if ui
                .add(
                    egui::Button::new(text)
                        .fill(egui::Color32::from_rgb(color[0], color[1], color[2])),
                )
                .clicked()
            {
                self.set_markup_color(color);
                ui.close();
            }
        }
        ui.horizontal(|ui| {
            ui.label("Personalizado");
            let mut color = self.preferences.markup_color;
            if ui.color_edit_button_srgb(&mut color).changed() {
                self.set_markup_color(color);
            }
        });
    }
    pub(super) fn menu_action(&mut self, ui: &mut egui::Ui, action: Action) {
        if ui
            .add_enabled(
                self.enabled(action),
                egui::Button::new(action.label()).shortcut_text(action.shortcut().unwrap_or("")),
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
                    ui.menu_button("Marcas", |ui| {
                        for action in [
                            Action::Highlight,
                            Action::Underline,
                            Action::UndoMarkup,
                            Action::SaveAs,
                        ] {
                            self.menu_action(ui, action);
                        }
                        ui.separator();
                        self.markup_colors(ui);
                    });
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
                            );
                        }
                    });
                });
            });
    }

    pub(super) fn search_bar(&mut self, ui: &mut egui::Ui) {
        if self.home {
            return;
        }
        if !self
            .tabs
            .get(self.active)
            .is_some_and(|tab| tab.search.open)
        {
            return;
        }
        let mut changed = false;
        let mut action = None;
        egui::Panel::top("find").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let tab = &mut self.tabs[self.active];
                ui.label("Buscar:");
                let response = ui.add(
                    egui::TextEdit::singleline(&mut tab.search.query)
                        .desired_width(240.0)
                        .hint_text("Texto del documento"),
                );
                if tab.search.focus {
                    response.request_focus();
                    tab.search.focus = false;
                }
                changed |= response.changed();
                changed |= ui
                    .checkbox(&mut tab.search.match_case, "Aa")
                    .on_hover_text("Distinguir mayúsculas")
                    .changed();
                changed |= ui
                    .checkbox(&mut tab.search.whole_word, "Palabra completa")
                    .changed();
                let count = tab.search.hits.len();
                ui.label(format!(
                    "{} / {count}",
                    if count == 0 { 0 } else { tab.search.index + 1 }
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
                if tab.search.limited { ui.label("Límite de 10 000 coincidencias").on_hover_text("Refina la búsqueda para consultar otros resultados. El límite controla la memoria de documentos extensos."); }
                if !tab.search.limited && tab.search.scanned < tab.search.total && !tab.search.query.is_empty() {
                    ui.spinner();
                    ui.label(format!(
                        "{}/{} páginas",
                        tab.search.scanned, tab.search.total
                    ));
                }
                if ui.small_button("×").clicked() {
                    tab.search.open = false;
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

    pub(super) fn sidebar_panel(&mut self, ui: &mut egui::Ui) {
        if self.home {
            return;
        }
        if !(if self.fullscreen {
            self.fullscreen_sidebar
        } else {
            self.preferences.sidebar_open
        }) {
            return;
        }
        egui::Panel::left("sidebar")
            .default_size(210.0)
            .size_range(165.0..=340.0)
            .show(ui, |ui| {
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
                        let Some(document) = &tab.state.document else {
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
                                                self.tabs[self.active].state.go_to(page, true);
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
                                            self.tabs[self.active].state.go_to(page, true);
                                        }
                                    });
                                }
                            });
                    }
                    Sidebar::Outline => {
                        let outline = self
                            .state()
                            .details
                            .as_ref()
                            .map(|d| d.outline.clone())
                            .unwrap_or_default();
                        if outline.is_empty() {
                            ui.label("El documento no contiene un índice incorporado.");
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (index, item) in outline.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add_space((item.depth.min(12) * 10) as f32);
                                    if ui
                                        .add_enabled(
                                            item.page.is_some(),
                                            egui::Button::new(&item.title).frame(false),
                                        )
                                        .on_hover_text(item.page.map_or_else(String::new, |p| {
                                            format!("Página {}", p + 1)
                                        }))
                                        .clicked()
                                        && let Some(page) = item.page
                                    {
                                        self.tabs[self.active].state.go_to(page, true);
                                    }
                                    ui.push_id(index, |_| {});
                                });
                            }
                        });
                    }
                    Sidebar::Favorites => {
                        let favorites = self.preferences.favorites.clone();
                        if favorites.is_empty() {
                            ui.label("Añade una página desde Paneles o Ctrl/⌘ + B.");
                        }
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for favorite in favorites {
                                if ui
                                    .button(&favorite.label)
                                    .on_hover_text(favorite.path.display().to_string())
                                    .clicked()
                                {
                                    let saved = StoredDocument {
                                        path: favorite.path.clone(),
                                        page: favorite.page,
                                        zoom: Zoom::FitWidth,
                                        layout: LayoutMode::Single,
                                        continuous: false,
                                        rotation: 0,
                                        colors: PageColors::Original,
                                    };
                                    self.open_document(favorite.path, Some(saved));
                                    if let Some(tab) = self.tabs.get_mut(self.active) {
                                        tab.state.go_to(favorite.page, true);
                                    }
                                }
                            }
                        });
                    }
                }
            });
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let palette = theme::palette(self.preferences.theme);
        egui::Panel::bottom("status")
            .frame(egui::Frame::NONE.fill(palette.surface).inner_margin(egui::Margin::symmetric(14,4)))
            .show(ui,|ui|ui.horizontal(|ui| {
                let text=if self.dialog.is_some() {"Selecciona un archivo"}
                    else if self.state().opening.is_some() {"Abriendo documento…"}
                    else if self.state().rendering {"Renderizando página…"}
                    else if self.state().document.is_some() {"Izq./Der. · Páginas o desplazar   ·   Arriba/Abajo · Desplazar"}
                    else {"Lectura local · Sin cuentas"};
                if self.state().opening.is_some() || self.state().rendering { ui.spinner(); }
                ui.label(RichText::new(text).small().color(palette.muted));
                ui.with_layout(Layout::right_to_left(Align::Center),|ui| {
                    ui.label(RichText::new(format!("{} {}",crate::branding::NAME,env!("CARGO_PKG_VERSION"))).small().color(palette.muted));
                    if let Some(key)=self.state().requested && self.pages.get(&key).is_some_and(|page|page.limited) {
                        ui.label(RichText::new("Resolución limitada").small())
                            .on_hover_text("Se limita la resolución para controlar la memoria y el tamaño máximo de textura.");
                    }
                });
            }));
    }

    pub(super) fn notices(&mut self, ui: &mut egui::Ui) {
        if let Some(error) = self.state().error.clone() {
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
                        if self.password_retry_available()
                            && ui.button("Introducir contraseña").clicked()
                            && let Some(tab) = self.tabs.get_mut(self.active)
                        {
                            tab.password_needed = true;
                        }
                        if error.kind == ErrorKind::Render
                            && ui.button("Reintentar").clicked()
                            && let Some(tab) = self.tabs.get_mut(self.active)
                        {
                            tab.state.invalidate();
                            self.pending_pages.clear();
                        }
                        if self.worker.is_some() && ui.small_button("Cerrar aviso").clicked() {
                            if let Some(tab) = self.tabs.get_mut(self.active) {
                                tab.state.error = None;
                            } else {
                                self.empty.error = None;
                            }
                        }
                    });
                });
        }
        let notice = self.config_warning.clone().or_else(|| self.notice.clone());
        if let Some(notice) = notice {
            egui::Panel::top("notice").show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(notice);
                    if ui.small_button("Cerrar").clicked() {
                        self.config_warning = None;
                        self.notice = None;
                    }
                });
            });
        }
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
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
                    tab.state.go_to(self.go_to_value.saturating_sub(1), true);
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
            let path = self.tabs.get(self.active).map(|t| t.path.clone());
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
                                self.preferences.native_titlebar,
                            ));
                        changed = true;
                    }
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
        if !tab.password_needed {
            return;
        }
        let mut submit = false;
        let mut cancel = false;
        egui::Window::new("Documento protegido")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(tab.path.file_name().unwrap_or_default().to_string_lossy());
                ui.label("Introduce la contraseña para abrir este documento.");
                let response = ui.add(egui::TextEdit::singleline(&mut tab.password).password(true));
                response.request_focus();
                ui.horizontal(|ui| {
                    submit = ui.button("Abrir").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    cancel = ui.button("Cancelar").clicked();
                });
            });
        if cancel {
            tab.password_needed = false;
            tab.password.clear();
        }
        if submit {
            tab.password_needed = false;
            tab.state.opening = Some(tab.id);
            tab.state.error = None;
            let password = std::mem::take(&mut tab.password);
            if let Some(worker) = &self.worker
                && let Err(error) =
                    worker.open_with_password(tab.id, tab.path.clone(), Some(password))
            {
                tab.state.finish_open(tab.id, Err(error));
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
