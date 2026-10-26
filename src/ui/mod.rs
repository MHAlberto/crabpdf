mod actions;
mod chrome;
mod events;
mod home;
mod icons;
mod input;
mod key_scroll;
mod motion;
mod panels;
mod raster;
mod requests;
mod selection_geometry;
mod session;
mod tab_view;
mod theme;
mod viewer;

use tab_view::Tab;

use crate::config::{Favorite, Preferences, ProfileSession, StoredDocument, Theme};
use crate::desktop::shell as platform;
use crabpdf_application::commands::Action;
use crabpdf_application::favorites::{self, FavoriteAction, FavoriteEffect};
use crabpdf_application::protocol::WorkerEvent;
use crabpdf_application::state::AppState;
use crabpdf_core::document::{
    DocumentId, ErrorKind, PageContent, ReaderError, RenderKey, SearchHit, Zoom,
};
use crabpdf_core::layout::LayoutMode;
use crabpdf_runtime::cache::ByteCache;
use crabpdf_runtime::paths::AppPaths;
use crabpdf_runtime::worker::PdfWorker;
use eframe::egui::{self, Vec2};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct DisplayPage {
    texture: egui::TextureHandle,
    size: Vec2,
    limited: bool,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sidebar {
    #[default]
    Thumbnails,
    Outline,
    Favorites,
}
fn consume_reader_key(ctx: &egui::Context, modifiers: egui::Modifiers, key: egui::Key) -> bool {
    ctx.input_mut(|input| {
        let exact = input.events.iter().any(|event| {
            matches!(event,egui::Event::Key { key:pressed,pressed:true,modifiers:actual,.. }
            if *pressed==key && actual.shift==modifiers.shift && actual.alt==modifiers.alt)
        });
        exact && input.consume_key(modifiers, key)
    })
}
pub struct ReaderApp {
    tabs: Vec<Tab>,
    active: usize,
    home: bool,
    reveal_tab: bool,
    library_frequent: bool,
    covers: ByteCache<PathBuf, egui::TextureHandle>,
    pending_covers: HashSet<PathBuf>,
    cover_errors: HashMap<PathBuf, String>,
    empty: AppState,
    next_id: u64,
    next_search: u64,
    closed: Vec<StoredDocument>,
    worker: Option<PdfWorker>,
    pages: ByteCache<RenderKey, DisplayPage>,
    content: ByteCache<(DocumentId, usize), PageContent>,
    pending_pages: HashSet<RenderKey>,
    pending_content: HashSet<(DocumentId, usize)>,
    frame_keys: Vec<RenderKey>,
    preferences: Preferences,
    profile: ProfileSession,
    app_paths: AppPaths,
    config_warning: Option<String>,
    save_due: Option<Instant>,
    dialog: Option<crate::desktop::dialogs::PendingDialog>,
    sidebar: Sidebar,
    toolbar_visible: bool,
    menu_visible: bool,
    fullscreen: bool,
    #[cfg(target_os = "windows")]
    fullscreen_restore_maximized: Option<bool>,
    fullscreen_sidebar: bool,
    fullscreen_controls: Option<egui::Rect>,
    presentation: bool,
    presentation_previous_fullscreen: Option<bool>,
    initial_presentation: bool,
    palette_open: bool,
    palette_query: String,
    palette_index: usize,
    palette_focus: bool,
    palette_reveal: bool,
    go_to_open: bool,
    go_to_value: usize,
    zoom_open: bool,
    zoom_value: u16,
    properties_open: bool,
    settings_open: bool,
    notice: Option<String>,
    notice_started: Option<(String, Instant)>,
    last_title: String,
    entrance: motion::Entrance,
    markup_button: Option<egui::Rect>,
    tab_drag: Option<u64>,
    tab_buttons: Vec<(u64, egui::Rect)>,
    favorite_buttons: Vec<(usize, egui::Rect)>,
    favorite_toggle: Option<egui::Rect>,
}
impl ReaderApp {
    pub fn set_markup_color(&mut self, color: [u8; 3]) {
        self.preferences.markup_color = color;
        self.save_due = Some(Instant::now() + Duration::from_millis(500));
    }
    pub fn scroll_offset(&self) -> Option<Vec2> {
        self.tabs.get(self.active).map(|tab| tab.view.scroll)
    }
    pub fn markup_button_rect(&self) -> Option<egui::Rect> {
        self.markup_button
    }
    pub fn markup_count(&self) -> usize {
        self.tabs
            .get(self.active)
            .map_or(0, |tab| tab.reader.local_marks)
    }
    pub fn password_prompt_active(&self) -> bool {
        self.tabs
            .get(self.active)
            .is_some_and(|tab| tab.reader.password_needed)
    }
    pub fn password_retry_available(&self) -> bool {
        self.tabs.get(self.active).is_some_and(|tab| {
            tab.supports_password()
                && tab
                    .reader
                    .state
                    .error
                    .as_ref()
                    .is_some_and(|error| error.kind == ErrorKind::Protected)
        })
    }
    pub fn selection_text(&mut self) -> Option<String> {
        self.selected_text()
    }
    /// Visible text position for native integration tests and accessibility automation.
    pub fn text_hit_position(&mut self) -> Option<egui::Pos2> {
        let tab = self.tabs.get(self.active)?;
        let id = tab.reader.id;
        let page = tab.reader.state.document.as_ref()?.page;
        let content = self.need_content(id, page)?;
        let tab = &self.tabs[self.active];
        let (_, layout) = tab.view.view_layout.as_ref()?;
        let placement = layout
            .placements
            .iter()
            .find(|placement| placement.page == page)?;
        let viewport = tab.view.viewport_rect?;
        content
            .glyphs
            .iter()
            .filter(|glyph| glyph.text.chars().any(char::is_alphanumeric))
            .filter_map(|glyph| glyph.bounds)
            .find_map(|bounds| {
                let bounds = bounds.rotated(tab.reader.state.rotation);
                let position = viewport.min
                    + egui::vec2(
                        placement.left - tab.view.scroll.x
                            + (bounds.left + bounds.right) * 0.5 * placement.size.width,
                        placement.top - tab.view.scroll.y
                            + (bounds.top + bounds.bottom) * 0.5 * placement.size.height,
                    );
                viewport.contains(position).then_some(position)
            })
    }
    pub fn with_launch(
        cc: &eframe::CreationContext<'_>,
        options: crate::cli::LaunchOptions,
    ) -> Self {
        let mut app = Self::with_config(
            cc,
            options.files.first().cloned(),
            options.config.or_else(crabpdf_runtime::paths::config_path),
        );
        for path in options.files.into_iter().skip(1) {
            app.open_path(path);
        }
        for tab in &mut app.tabs {
            tab.reader.startup = Some(options.view.clone());
        }
        if options.fullscreen {
            app.fullscreen = true;
            app.sync_fullscreen(&cc.egui_ctx);
        }
        app.initial_presentation = options.presentation;
        app
    }
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        Self::with_config(cc, initial_path, crabpdf_runtime::paths::config_path())
    }
    pub fn with_config(
        cc: &eframe::CreationContext<'_>,
        initial_path: Option<PathBuf>,
        config_path: Option<PathBuf>,
    ) -> Self {
        let repaint = cc.egui_ctx.clone();
        let services = crate::desktop::bootstrap::DesktopServices::start(config_path, move || {
            repaint.request_repaint()
        });
        let preferences = services.preferences;
        theme::apply(&cc.egui_ctx, preferences.theme);
        cc.egui_ctx
            .options_mut(|options| options.zoom_with_keyboard = false);
        if !cfg!(target_os = "macos") {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Decorations(
                    preferences.native_titlebar,
                ));
        }
        let mut empty = AppState::default();
        empty.error = services.worker_error;
        let session = if initial_path.is_none() && preferences.restore_session {
            preferences.session.clone()
        } else {
            Vec::new()
        };
        let mut app = Self {
            tabs: Vec::new(),
            active: 0,
            home: true,
            reveal_tab: true,
            library_frequent: false,
            covers: ByteCache::new(8 * 1024 * 1024),
            pending_covers: HashSet::new(),
            cover_errors: HashMap::new(),
            empty,
            next_id: 0,
            next_search: 0,
            closed: Vec::new(),
            worker: services.worker,
            pages: ByteCache::new(if cfg!(target_pointer_width = "32") {
                32 * 1024 * 1024
            } else {
                64 * 1024 * 1024
            }),
            content: ByteCache::new(16 * 1024 * 1024),
            pending_pages: HashSet::new(),
            pending_content: HashSet::new(),
            frame_keys: Vec::new(),
            preferences,
            profile: services.profile,
            app_paths: services.paths,
            config_warning: services.config_warning,
            save_due: None,
            dialog: None,
            sidebar: Sidebar::default(),
            toolbar_visible: true,
            menu_visible: true,
            fullscreen: false,
            #[cfg(target_os = "windows")]
            fullscreen_restore_maximized: None,
            fullscreen_sidebar: false,
            fullscreen_controls: None,
            presentation: false,
            presentation_previous_fullscreen: None,
            initial_presentation: false,
            palette_open: false,
            palette_query: String::new(),
            palette_index: 0,
            palette_focus: false,
            palette_reveal: false,
            go_to_open: false,
            go_to_value: 1,
            zoom_open: false,
            zoom_value: 100,
            properties_open: false,
            settings_open: false,
            notice: None,
            notice_started: None,
            entrance: motion::Entrance::default(),
            markup_button: None,
            tab_drag: None,
            tab_buttons: Vec::new(),
            favorite_buttons: Vec::new(),
            favorite_toggle: None,
            last_title: String::new(),
        };
        for saved in session.into_iter().take(64) {
            app.open_document(saved.path.clone(), Some(saved));
        }
        if let Some(path) = initial_path {
            app.open_path(path);
        }
        app
    }
    pub fn state(&self) -> &AppState {
        if self.home {
            return &self.empty;
        }
        self.tabs
            .get(self.active)
            .map_or(&self.empty, |tab| &tab.reader.state)
    }
    pub fn has_current_render(&self) -> bool {
        self.state()
            .requested
            .is_some_and(|key| self.pages.contains(&key))
            && !self.state().rendering
    }
    pub fn prefetch_ready(&self, page: usize) -> bool {
        self.state().requested.is_some_and(|key| {
            self.pages.contains(&RenderKey {
                page,
                region: None,
                bitmap_side_limit: key.bitmap_side_limit.min(768),
                ..key
            })
        })
    }
    pub fn tab_button_rect(&self, index: usize) -> Option<egui::Rect> {
        let id = self.tabs.get(index)?.reader.id;
        self.tab_buttons
            .iter()
            .find(|(tab, _)| *tab == id)
            .map(|(_, rect)| *rect)
    }
    pub fn favorite_button_rect(&self, page: usize) -> Option<egui::Rect> {
        self.favorite_buttons
            .iter()
            .find(|(p, _)| *p == page)
            .map(|(_, rect)| *rect)
    }
    pub fn favorite_toggle_rect(&self) -> Option<egui::Rect> {
        self.favorite_toggle
    }
    fn favorite_action(&mut self, action: FavoriteAction) {
        let save_now = matches!(action, FavoriteAction::Toggle { .. });
        let Some(document) = self
            .tabs
            .get(self.active)
            .filter(|_| !self.home)
            .and_then(|tab| tab.reader.state.document.as_ref())
        else {
            return;
        };
        let effect = favorites::apply(&mut self.preferences.favorites, &document.info, action);
        match effect {
            FavoriteEffect::Changed => {
                if save_now {
                    self.save_preferences();
                } else {
                    self.schedule_save();
                }
            }
            FavoriteEffect::Navigate(page) => self.tabs[self.active].go_to(page, true),
            FavoriteEffect::Unchanged => {}
        }
    }
    pub fn current_favorites(&self) -> Vec<Favorite> {
        if self.home {
            return Vec::new();
        }
        self.tabs
            .get(self.active)
            .filter(|tab| tab.reader.state.document.is_some())
            .map_or_else(Vec::new, |tab| {
                self.preferences
                    .favorites_for(&tab.reader.path)
                    .into_iter()
                    .cloned()
                    .collect()
            })
    }
    /// Reapply decorations after switching mode: egui-winit also refreshes
    /// Windows' undecorated shadow here, which must be disabled in fullscreen.
    fn sync_fullscreen(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "windows")]
        if self.fullscreen && self.fullscreen_restore_maximized.is_none() {
            let maximized = ctx.input(|input| input.viewport().maximized.unwrap_or(false));
            self.fullscreen_restore_maximized = Some(maximized);
            if maximized {
                // winit's maximized WM_NCCALCSIZE path uses rcWork rather than
                // rcMonitor, leaving the taskbar strip outside the client area.
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        #[cfg(target_os = "windows")]
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(
                !self.fullscreen && self.preferences.native_titlebar,
            ));
            if !self.fullscreen && self.fullscreen_restore_maximized.take() == Some(true) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
        }
    }
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
    pub fn screen_modes(&self) -> (bool, bool) {
        (self.fullscreen, self.presentation)
    }
    pub fn reading_viewport(&self) -> Option<egui::Rect> {
        self.tabs.get(self.active)?.view.viewport_rect
    }
    pub fn fullscreen_controls_rect(&self) -> Option<egui::Rect> {
        self.fullscreen_controls
    }
    pub fn search_query(&self) -> Option<&str> {
        Some(&self.tabs.get(self.active)?.reader.search.query)
    }
    pub fn search_error(&self) -> Option<&ReaderError> {
        if self.home {
            return None;
        }
        self.tabs.get(self.active)?.reader.search.error.as_ref()
    }
    pub fn search_progress(&self) -> (usize, usize, usize) {
        self.tabs.get(self.active).map_or((0, 0, 0), |tab| {
            (
                tab.reader.search.hits.len(),
                tab.reader.search.scanned,
                tab.reader.search.total,
            )
        })
    }
    pub fn current_search_hit(&self) -> Option<&SearchHit> {
        let tab = self.tabs.get(self.active)?;
        tab.reader.search.current()
    }
    pub fn current_page_viewport(&self) -> Option<crabpdf_core::document::Bounds> {
        let tab = self.tabs.get(self.active)?;
        if tab.reader.state.scroll_to_page
            || tab.view.reveal.is_some()
            || tab.reader.pending_position.is_some()
            || tab.view.scroll_motion.is_some()
        {
            return None;
        }
        let (spec, layout) = tab.view.view_layout.as_ref()?;
        if spec.revision != tab.reader.layout_revision {
            return None;
        }
        let page = tab.reader.state.document.as_ref()?.page;
        let placement = layout.placements.iter().find(|p| p.page == page)?;
        Some(crabpdf_core::document::Bounds {
            left: (tab.view.scroll.x - placement.left) / placement.size.width,
            top: (tab.view.scroll.y - placement.top) / placement.size.height,
            right: (tab.view.scroll.x + tab.view.viewport.x - placement.left)
                / placement.size.width,
            bottom: (tab.view.scroll.y + tab.view.viewport.y - placement.top)
                / placement.size.height,
        })
    }
    pub fn navigate(&mut self, forward: bool) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.reader.pending_position = None;
            tab.view.scroll_motion = None;
            tab.view.navigation_wait = None;
            tab.reader.state.navigate(forward);
            tab.reader.selection.clear();
        }
        self.schedule_save();
    }
    pub fn set_zoom(&mut self, zoom: Zoom) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.zoom_at(zoom, None);
        }
        self.schedule_save();
    }
    pub fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.preferences.theme = match self.preferences.theme {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        };
        theme::apply(ctx, self.preferences.theme);
        self.save_preferences();
    }
}
impl eframe::App for ReaderApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Some(result) = self.profile.poll() {
            self.config_warning = result.err();
        }
        self.poll_dialog(ctx);
        self.poll_worker(ctx);
        if self.presentation && self.state().document.is_none() {
            self.leave_presentation(ctx);
        }
        if self.initial_presentation && self.state().document.is_some() {
            self.initial_presentation = false;
            self.execute(Action::Presentation, ctx);
        }
        self.shortcuts(ctx);
        if let Some(tab) = self.tabs.get_mut(self.active)
            && let Some(due) = tab.view.search.due
        {
            if due <= Instant::now() {
                tab.view.search.due = None;
                if let Some(worker) = &self.worker
                    && let Some(request) = tab.reader.search.request()
                    && let Err(error) = worker.search(request)
                {
                    tab.reader.state.error = Some(error);
                }
            } else {
                ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
            }
        }
        if let Some(due) = self.save_due {
            if due <= Instant::now() {
                self.save_preferences();
            } else {
                ctx.request_repaint_after(due.saturating_duration_since(Instant::now()));
            }
        }
        let title = self.state().document.as_ref().map_or_else(
            || crate::branding::NAME.into(),
            |d| format!("{} · {}", d.info.file_name(), crate::branding::NAME),
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame_keys.clear();
        self.markup_button = None;
        self.tab_buttons.clear();
        self.favorite_buttons.clear();
        self.favorite_toggle = None;
        let duration = if self.preferences.reduce_motion {
            0.0
        } else {
            motion::DURATION
        };
        if ui.style().animation_time != duration {
            for theme in [egui::Theme::Light, egui::Theme::Dark] {
                ui.ctx()
                    .style_mut_of(theme, |style| style.animation_time = duration);
            }
            ui.style_mut().animation_time = duration;
        }
        let fullscreen_reading = self.fullscreen && self.state().document.is_some();
        if fullscreen_reading {
            self.tab_drag = None;
        }
        if !fullscreen_reading {
            self.titlebar(ui);
            if self.menu_visible {
                self.menu(ui);
            }
            if self.toolbar_visible {
                self.toolbar(ui);
            }
            self.search_bar(ui);
            self.notices(ui);
            self.sidebar_panel(ui);
        } else {
            // Search and errors appear only when requested; ordinary reading
            // uses the entire viewport and keeps the windowed preferences.
            self.search_bar(ui);
            self.notices(ui);
            if !self.presentation {
                self.sidebar_panel(ui);
            }
        }
        self.document_view(ui);
        if fullscreen_reading && !self.presentation {
            self.fullscreen_reading_controls(ui.ctx());
        } else {
            self.fullscreen_controls = None;
        }
        self.dialogs(ui.ctx());
        self.resize_edges(ui.ctx());
        if let Some(worker) = &self.worker {
            let _ = worker.retain_renders(&self.frame_keys);
        }
        self.pending_pages
            .retain(|key| self.frame_keys.contains(key));
        let dropped = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| PathBuf::from(file.path()))
                .collect::<Vec<_>>()
        });
        for path in dropped {
            self.open_path(path);
        }
    }
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_preferences();
        self.config_warning = self.profile.finish().err();
    }
}
