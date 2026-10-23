mod chrome;
mod home;
mod icons;
mod motion;
mod panels;
mod theme;
mod viewer;

use crate::app_paths::AppPaths;
use crate::cache::ByteCache;
use crate::commands::Action;
use crate::config::{Favorite, PreferenceWriter, Preferences, StoredDocument, Theme};
use crate::document::{
    DocumentId, ErrorKind, PageContent, ReaderError, RenderKey, SearchHit, SearchRequest, Zoom,
};
use crate::layout::LayoutMode;
use crate::platform;
use crate::state::AppState;
use crate::worker::{PdfWorker, WorkerEvent};
use eframe::egui::{self, Vec2};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::{Duration, Instant};

struct DisplayPage {
    texture: egui::TextureHandle,
    size: Vec2,
    zoom: f32,
    limited: bool,
}
#[derive(Default)]
struct SearchState {
    open: bool,
    focus: bool,
    query: String,
    match_case: bool,
    whole_word: bool,
    generation: u64,
    hits: Vec<SearchHit>,
    index: usize,
    scanned: usize,
    total: usize,
    limited: bool,
    due: Option<Instant>,
}
struct Tab {
    id: DocumentId,
    path: PathBuf,
    state: AppState,
    search: SearchState,
    selection: Option<(usize, usize, usize)>,
    local_marks: usize,
    marking: bool,
    select_all_pending: bool,
    scroll: Vec2,
    viewport: Vec2,
    viewport_rect: Option<egui::Rect>,
    zoom_anchor: Option<crate::layout::ReadingAnchor>,
    pending_position: Option<crate::reading_position::ReadingPosition>,
    scroll_bottom: bool,
    scroll_motion: Option<motion::ScrollMotion>,
    displayed_page: Option<usize>,
    navigation_wait: Option<(usize, f64)>,
    reveal: Option<(usize, crate::document::Bounds)>,
    password_needed: bool,
    password: String,
    restore: Option<StoredDocument>,
    startup: Option<crate::cli::InitialView>,
    layout_revision: u64,
    view_layout: Option<(crate::layout::LayoutSpec, Arc<crate::layout::PageLayout>)>,
}
impl Tab {
    fn go_to(&mut self, page: usize, remember: bool) {
        self.pending_position = None;
        self.scroll_motion = None;
        self.navigation_wait = None;
        self.state.go_to(page, remember);
    }
    fn supports_password(&self) -> bool {
        matches!(
            crate::formats::Format::detect(&self.path),
            Ok(crate::formats::Format::Pdf)
        )
    }

    fn new(id: DocumentId, path: PathBuf, restore: Option<StoredDocument>) -> Self {
        let mut state = AppState::default();
        state.begin_open_with_id(id);
        Self {
            id,
            path,
            state,
            search: SearchState::default(),
            selection: None,
            local_marks: 0,
            marking: false,
            select_all_pending: false,
            scroll: Vec2::ZERO,
            viewport: Vec2::ZERO,
            viewport_rect: None,
            zoom_anchor: None,
            pending_position: None,
            scroll_bottom: false,
            scroll_motion: None,
            displayed_page: None,
            navigation_wait: None,
            reveal: None,
            password_needed: false,
            password: String::new(),
            restore,
            startup: None,
            layout_revision: 0,
            view_layout: None,
        }
    }
    fn stored(&self) -> Option<StoredDocument> {
        let document = self.state.document.as_ref()?;
        Some(StoredDocument {
            path: self.path.clone(),
            page: document.page,
            zoom: self.state.zoom,
            layout: self.state.layout,
            continuous: self.state.continuous,
            rotation: self.state.rotation,
            colors: self.state.colors,
            manga: self.state.manga,
            position: self.pending_position.or_else(|| {
                let (_, layout) = self.view_layout.as_ref()?;
                crate::reading_position::ReadingPosition::capture(
                    layout,
                    (self.scroll.x, self.scroll.y),
                    (self.viewport.x, self.viewport.y),
                )
                .filter(|position| {
                    self.state
                        .page_sizes
                        .get(position.page())
                        .is_some_and(Option::is_some)
                })
            }),
        })
    }
    fn anchor(&self, pointer: Option<egui::Pos2>) -> Option<crate::layout::ReadingAnchor> {
        let (_, layout) = self.view_layout.as_ref()?;
        let screen = pointer
            .zip(self.viewport_rect)
            .filter(|(pointer, rect)| rect.contains(*pointer))
            .map_or(self.viewport * 0.5, |(pointer, rect)| pointer - rect.min);
        let point = self.scroll + screen;
        let placement = layout
            .placements
            .iter()
            .find(|p| {
                point.x >= p.left
                    && point.x <= p.left + p.size.width
                    && point.y >= p.top
                    && point.y <= p.top + p.size.height
            })
            .or_else(|| {
                layout.placements.iter().find(|p| {
                    self.state
                        .document
                        .as_ref()
                        .is_some_and(|document| p.page == document.page)
                })
            })?;
        Some(crate::layout::ReadingAnchor {
            page: placement.page,
            x: ((point.x - placement.left) / placement.size.width).clamp(0.0, 1.0),
            y: ((point.y - placement.top) / placement.size.height).clamp(0.0, 1.0),
            screen_x: screen.x,
            screen_y: screen.y,
        })
    }
    fn zoom_at(&mut self, zoom: Zoom, pointer: Option<egui::Pos2>) {
        self.pending_position = None;
        self.scroll_motion = None;
        self.navigation_wait = None;
        if self.state.zoom == zoom {
            return;
        }
        let anchor = self.anchor(pointer);
        self.state.set_zoom(zoom);
        self.zoom_anchor = anchor;
        if self.zoom_anchor.is_some() {
            self.state.scroll_to_page = false;
        }
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Sidebar {
    #[default]
    Thumbnails,
    Outline,
    Favorites,
}
struct RepaintWake(egui::Context);
fn consume_reader_key(ctx: &egui::Context, modifiers: egui::Modifiers, key: egui::Key) -> bool {
    ctx.input_mut(|input| {
        let exact = input.events.iter().any(|event| {
            matches!(event,egui::Event::Key { key:pressed,pressed:true,modifiers:actual,.. }
            if *pressed==key && actual.shift==modifiers.shift && actual.alt==modifiers.alt)
        });
        exact && input.consume_key(modifiers, key)
    })
}
impl Wake for RepaintWake {
    fn wake(self: Arc<Self>) {
        self.0.request_repaint();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.request_repaint();
    }
}
type DialogFuture = Pin<Box<dyn Future<Output = Option<Vec<rfd::FileHandle>>> + Send>>;
enum Dialog {
    Open(DialogFuture),
    Save(DocumentId, DialogFuture),
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
    preference_writer: Option<PreferenceWriter>,
    config_path: Option<PathBuf>,
    app_paths: AppPaths,
    config_warning: Option<String>,
    save_due: Option<Instant>,
    dialog: Option<Dialog>,
    sidebar: Sidebar,
    toolbar_visible: bool,
    menu_visible: bool,
    fullscreen: bool,
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
}
impl ReaderApp {
    pub fn set_markup_color(&mut self, color: [u8; 3]) {
        self.preferences.markup_color = color;
        self.save_due = Some(Instant::now() + Duration::from_millis(500));
    }
    pub fn scroll_offset(&self) -> Option<Vec2> {
        self.tabs.get(self.active).map(|tab| tab.scroll)
    }
    pub fn markup_button_rect(&self) -> Option<egui::Rect> {
        self.markup_button
    }
    pub fn markup_count(&self) -> usize {
        self.tabs.get(self.active).map_or(0, |tab| tab.local_marks)
    }
    pub fn password_prompt_active(&self) -> bool {
        self.tabs
            .get(self.active)
            .is_some_and(|tab| tab.password_needed)
    }
    pub fn password_retry_available(&self) -> bool {
        self.tabs.get(self.active).is_some_and(|tab| {
            tab.supports_password()
                && tab
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
        let id = tab.id;
        let page = tab.state.document.as_ref()?.page;
        let content = self.need_content(id, page)?;
        let tab = &self.tabs[self.active];
        let (_, layout) = tab.view_layout.as_ref()?;
        let placement = layout
            .placements
            .iter()
            .find(|placement| placement.page == page)?;
        let viewport = tab.viewport_rect?;
        content
            .glyphs
            .iter()
            .filter(|glyph| glyph.text.chars().any(char::is_alphanumeric))
            .filter_map(|glyph| glyph.bounds)
            .find_map(|bounds| {
                let bounds = bounds.rotated(tab.state.rotation);
                let position = viewport.min
                    + egui::vec2(
                        placement.left - tab.scroll.x
                            + (bounds.left + bounds.right) * 0.5 * placement.size.width,
                        placement.top - tab.scroll.y
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
            options.config.or_else(platform::config_path),
        );
        for path in options.files.into_iter().skip(1) {
            app.open_path(path);
        }
        for tab in &mut app.tabs {
            tab.startup = Some(options.view.clone());
        }
        if options.fullscreen {
            app.fullscreen = true;
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        app.initial_presentation = options.presentation;
        app
    }
    pub fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        Self::with_config(cc, initial_path, platform::config_path())
    }
    pub fn with_config(
        cc: &eframe::CreationContext<'_>,
        initial_path: Option<PathBuf>,
        config_path: Option<PathBuf>,
    ) -> Self {
        let app_paths = AppPaths::from_environment(config_path.clone());
        let (preferences, mut config_warning) = match config_path.as_deref().map(Preferences::load)
        {
            Some(Ok(p)) => (p, None),
            Some(Err(e)) => (Preferences::default(), Some(e)),
            None => (Preferences::default(), None),
        };
        let preference_writer = config_path.as_ref().and_then(|path| {
            let repaint = cc.egui_ctx.clone();
            match PreferenceWriter::start(path.clone(), preferences.clone(), move || {
                repaint.request_repaint()
            }) {
                Ok(writer) => Some(writer),
                Err(error) => {
                    config_warning = Some(error);
                    None
                }
            }
        });
        theme::apply(&cc.egui_ctx, preferences.theme);
        cc.egui_ctx
            .options_mut(|options| options.zoom_with_keyboard = false);
        if !cfg!(target_os = "macos") {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Decorations(
                    preferences.native_titlebar,
                ));
        }
        let repaint = cc.egui_ctx.clone();
        let mut empty = AppState::default();
        let worker =
            match PdfWorker::spawn_with_paths(move || repaint.request_repaint(), app_paths.clone())
            {
                Ok(worker) => Some(worker),
                Err(error) => {
                    empty.error = Some(error);
                    None
                }
            };
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
            worker,
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
            preference_writer,
            config_path,
            app_paths,
            config_warning,
            save_due: None,
            dialog: None,
            sidebar: Sidebar::default(),
            toolbar_visible: true,
            menu_visible: true,
            fullscreen: false,
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
            .map_or(&self.empty, |tab| &tab.state)
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
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
    pub fn screen_modes(&self) -> (bool, bool) {
        (self.fullscreen, self.presentation)
    }
    pub fn reading_viewport(&self) -> Option<egui::Rect> {
        self.tabs.get(self.active)?.viewport_rect
    }
    pub fn fullscreen_controls_rect(&self) -> Option<egui::Rect> {
        self.fullscreen_controls
    }
    pub fn search_query(&self) -> Option<&str> {
        Some(&self.tabs.get(self.active)?.search.query)
    }
    pub fn search_progress(&self) -> (usize, usize, usize) {
        self.tabs.get(self.active).map_or((0, 0, 0), |tab| {
            (tab.search.hits.len(), tab.search.scanned, tab.search.total)
        })
    }
    pub fn current_search_hit(&self) -> Option<&SearchHit> {
        let tab = self.tabs.get(self.active)?;
        tab.search.hits.get(tab.search.index)
    }
    pub fn current_page_viewport(&self) -> Option<crate::document::Bounds> {
        let tab = self.tabs.get(self.active)?;
        if tab.state.scroll_to_page
            || tab.reveal.is_some()
            || tab.pending_position.is_some()
            || tab.scroll_motion.is_some()
        {
            return None;
        }
        let (spec, layout) = tab.view_layout.as_ref()?;
        if spec.revision != tab.layout_revision {
            return None;
        }
        let page = tab.state.document.as_ref()?.page;
        let placement = layout.placements.iter().find(|p| p.page == page)?;
        Some(crate::document::Bounds {
            left: (tab.scroll.x - placement.left) / placement.size.width,
            top: (tab.scroll.y - placement.top) / placement.size.height,
            right: (tab.scroll.x + tab.viewport.x - placement.left) / placement.size.width,
            bottom: (tab.scroll.y + tab.viewport.y - placement.top) / placement.size.height,
        })
    }
    pub fn open_path(&mut self, path: PathBuf) {
        self.open_document(path, None);
    }
    fn open_document(&mut self, path: PathBuf, restore: Option<StoredDocument>) {
        self.open_document_inner(path, restore, false);
    }
    fn open_document_inner(
        &mut self,
        path: PathBuf,
        restore: Option<StoredDocument>,
        duplicate: bool,
    ) {
        self.home = false;
        self.reveal_tab = true;
        let path = path.canonicalize().unwrap_or(path);
        if !duplicate && let Some(index) = self.tabs.iter().position(|tab| tab.path == path) {
            self.active = index;
            if let Some(saved) = restore {
                self.tabs[index].go_to(saved.page, true);
            }
            return;
        }
        if self.worker.is_none() {
            return;
        }
        if self.tabs.len() >= 64 {
            self.error(ReaderError::new(
                ErrorKind::File,
                "Cierra una pestaña antes de abrir más documentos.",
            ));
            return;
        }
        self.next_id += 1;
        let id = self.next_id;
        let saved = restore.or_else(|| {
            self.preferences
                .recent
                .iter()
                .find(|entry| entry.path == path)
                .cloned()
        });
        self.tabs.push(Tab::new(id, path.clone(), saved));
        self.active = self.tabs.len() - 1;
        if let Some(worker) = &self.worker
            && let Err(error) = worker.open(id, path)
        {
            self.tabs[self.active].state.finish_open(id, Err(error));
        }
    }
    fn close_index(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if let Some(saved) = self.tabs[index].stored() {
            self.closed.push(saved.clone());
            if self.closed.len() > 30 {
                self.closed.remove(0);
            }
            self.preferences.remember(saved);
        }
        let tab = self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.home = true;
        }
        if let Some(worker) = &self.worker
            && let Err(error) = worker.close(tab.id)
        {
            self.empty.error = Some(error);
        }
        self.pages.retain(|key| key.document_id != tab.id);
        self.content.retain(|(id, _)| *id != tab.id);
        self.pending_pages.retain(|key| key.document_id != tab.id);
        self.pending_content.retain(|(id, _)| *id != tab.id);
        if index < self.active {
            self.active -= 1;
        } else {
            self.active = self.active.min(self.tabs.len().saturating_sub(1));
        }
        self.save_preferences();
    }
    pub fn navigate(&mut self, forward: bool) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.pending_position = None;
            tab.scroll_motion = None;
            tab.navigation_wait = None;
            tab.state.navigate(forward);
            tab.selection = None;
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
    fn error(&mut self, error: ReaderError) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.state.error = Some(error);
        } else {
            self.empty.error = Some(error);
        }
    }
    fn schedule_save(&mut self) {
        self.save_due = Some(Instant::now() + Duration::from_millis(500));
    }
    fn save_preferences(&mut self) {
        self.preferences.session = self.tabs.iter().filter_map(Tab::stored).collect();
        for saved in self.preferences.session.clone() {
            if let Some(old) = self
                .preferences
                .recent
                .iter_mut()
                .find(|r| r.path == saved.path)
            {
                *old = saved;
            }
        }
        if let Some(writer) = &self.preference_writer {
            self.config_warning = writer.request(self.preferences.clone()).err();
        } else if let Some(path) = &self.config_path {
            self.config_warning = self.preferences.save(path).err();
        }
        self.save_due = None;
    }
    fn open_dialog(&mut self) {
        if self.dialog.is_none() && self.worker.is_some() {
            let future = rfd::AsyncFileDialog::new()
                .set_title("Abrir documentos")
                .add_filter("Documentos compatibles", crate::formats::EXTENSIONS)
                .add_filter("PDF", &["pdf", "ai"])
                .add_filter(
                    "Libros",
                    &[
                        "epub", "mobi", "azw", "azw3", "azw4", "prc", "pdb", "fb2", "fb2z", "fbz",
                        "zfb2",
                    ],
                )
                .add_filter(
                    "Cómics",
                    &["cbz", "cbr", "cb7", "cbt", "zip", "rar", "7z", "tar"],
                )
                .add_filter(
                    "DjVu / XPS / CHM",
                    &["djvu", "djv", "xps", "oxps", "xod", "dwfx", "chm"],
                )
                .add_filter("Todos los archivos", &["*"])
                .pick_files();
            self.dialog = Some(Dialog::Open(Box::pin(future)));
        }
    }
    fn save_dialog(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        if let Some(tab) = self.tabs.get(self.active) {
            let name = tab.path.with_extension("pdf");
            let name = name
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let future = rfd::AsyncFileDialog::new()
                .set_title("Guardar copia PDF con las anotaciones")
                .add_filter("PDF", &["pdf"])
                .set_file_name(name)
                .save_file();
            self.dialog = Some(Dialog::Save(
                tab.id,
                Box::pin(async move { future.await.map(|file| vec![file]) }),
            ));
        }
    }
    fn poll_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        let waker = Waker::from(Arc::new(RepaintWake(ctx.clone())));
        let mut task = Context::from_waker(&waker);
        let (kind, poll) = match dialog {
            Dialog::Open(f) => (None, f.as_mut().poll(&mut task)),
            Dialog::Save(id, f) => (Some(*id), f.as_mut().poll(&mut task)),
        };
        if let Poll::Ready(files) = poll {
            self.dialog = None;
            if let Some(files) = files {
                for file in files {
                    if let Some(id) = kind {
                        if let Some(worker) = &self.worker
                            && let Err(e) = worker.save(id, file.path().to_owned())
                        {
                            self.error(e);
                        }
                    } else {
                        self.open_path(file.path().to_owned());
                    }
                }
            }
        }
    }
    fn poll_worker(&mut self, ctx: &egui::Context) {
        loop {
            let event = match self.worker.as_ref().map(|worker| worker.events.try_recv()) {
                Some(Ok(event)) => event,
                Some(Err(std::sync::mpsc::TryRecvError::Empty)) | None => break,
                Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                    self.worker = None;
                    self.error(ReaderError::new(
                        ErrorKind::Worker,
                        "El motor PDF se detuvo. Reinicia la aplicación.",
                    ));
                    break;
                }
            };
            match event {
                WorkerEvent::Cover(path, result) => {
                    self.pending_covers.remove(&path);
                    if !self
                        .preferences
                        .recent
                        .iter()
                        .any(|saved| saved.path == path)
                    {
                        continue;
                    }
                    match result {
                        Ok(cover) => {
                            let image = egui::ColorImage::from_rgba_unmultiplied(
                                [cover.width, cover.height],
                                &cover.rgba,
                            );
                            let texture = ctx.load_texture(
                                format!("cover-{}", path.display()),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            self.covers
                                .insert(path, Arc::new(texture), cover.rgba.len());
                        }
                        Err(error) => {
                            self.cover_errors.insert(path, error.to_string());
                        }
                    }
                }
                WorkerEvent::Opened(id, result) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        let protected = result
                            .as_ref()
                            .is_err_and(|e| e.kind == ErrorKind::Protected)
                            && tab.supports_password();
                        tab.state.finish_open(id, result);
                        tab.password_needed = protected;
                        if tab.state.document.is_some() {
                            if let Some(saved) = tab.restore.take() {
                                tab.state.set_zoom(saved.zoom);
                                tab.state.layout = saved.layout;
                                tab.state.continuous = saved.continuous;
                                tab.state.rotation = saved.rotation % 4;
                                tab.state.colors = saved.colors;
                                tab.state.manga = saved.manga;
                                let count =
                                    tab.state.document.as_ref().map_or(0, |d| d.info.page_count);
                                tab.pending_position = saved
                                    .position
                                    .filter(|p| p.anchor((1.0, 1.0), count).is_some());
                                tab.state.go_to(
                                    tab.pending_position.map_or(saved.page, |p| p.page()),
                                    false,
                                );
                            }
                            if let Some(view) = tab.startup.take() {
                                if view.page.is_some()
                                    || view.zoom.is_some()
                                    || view.layout.is_some()
                                    || view.continuous.is_some()
                                    || view.manga.is_some()
                                    || view.rotation.is_some()
                                    || view.search.is_some()
                                {
                                    tab.pending_position = None;
                                }
                                if let Some(zoom) = view.zoom {
                                    tab.state.set_zoom(zoom);
                                }
                                if let Some(layout) = view.layout {
                                    tab.state.layout = layout;
                                }
                                if let Some(continuous) = view.continuous {
                                    tab.state.continuous = continuous;
                                }
                                if let Some(manga) = view.manga {
                                    tab.state.manga = manga;
                                }
                                if let Some(rotation) = view.rotation {
                                    tab.state.rotation = rotation;
                                }
                                if let Some(page) = view.page {
                                    tab.state.go_to(page, false);
                                }
                                if let Some(query) = view.search {
                                    self.next_search += 1;
                                    tab.search.query = query;
                                    tab.search.open = true;
                                    tab.search.generation = self.next_search;
                                    tab.search.total = tab
                                        .state
                                        .document
                                        .as_ref()
                                        .map_or(0, |d| d.info.page_count);
                                    tab.search.due = Some(Instant::now());
                                }
                            }
                            if let Some(saved) = tab.stored() {
                                self.preferences.remember(saved);
                                self.preferences.record_open(&tab.path);
                            }
                            self.schedule_save();
                        }
                    }
                }
                WorkerEvent::Details(id, result) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        match result {
                            Ok(details) => {
                                if let Some((_, warning)) = details
                                    .properties
                                    .iter()
                                    .find(|(name, _)| name == "Aviso de anotaciones")
                                {
                                    self.notice = Some(warning.clone());
                                }
                                tab.local_marks = details
                                    .properties
                                    .iter()
                                    .find(|(name, _)| name == "Marcas locales")
                                    .and_then(|(_, count)| count.parse().ok())
                                    .unwrap_or(0);
                                if let Some((_, title)) =
                                    details.properties.iter().find(|(name, _)| name == "Título")
                                    && let Some(entry) = self
                                        .preferences
                                        .library
                                        .iter_mut()
                                        .find(|entry| entry.path == tab.path)
                                {
                                    entry.title = title.clone();
                                }
                                if let Some(size) = tab.state.page_sizes.get_mut(0) {
                                    *size = Some(details.first_page_size);
                                }
                                tab.layout_revision += 1;
                                tab.state.details = Some(details);
                            }
                            Err(error) => tab.state.error = Some(error),
                        }
                    }
                }
                WorkerEvent::Rendered(key, result) => {
                    self.pending_pages.remove(&key);
                    if !self.tabs.iter().any(|tab| {
                        tab.id == key.document_id && tab.state.content_revision == key.revision
                    }) {
                        continue;
                    }
                    match result {
                        Ok(page) => {
                            if let Some(tab) =
                                self.tabs.iter_mut().find(|tab| tab.id == key.document_id)
                            {
                                if let Some(size) = tab.state.page_sizes.get_mut(key.page)
                                    && *size != Some(page.page_size)
                                {
                                    *size = Some(page.page_size);
                                    tab.layout_revision += 1;
                                }
                                if tab.state.requested == Some(key) {
                                    tab.state.rendering = false;
                                }
                            }
                            let image = egui::ColorImage::from_rgba_unmultiplied(
                                [page.width, page.height],
                                &page.rgba,
                            );
                            let texture = ctx.load_texture(
                                format!("pdf-{}-{}", key.document_id, key.page),
                                image,
                                egui::TextureOptions::LINEAR,
                            );
                            let display = Arc::new(DisplayPage {
                                texture,
                                size: egui::vec2(page.logical_width, page.logical_height),
                                zoom: page.zoom_percent,
                                limited: page.resolution_limited,
                            });
                            self.pages.insert(key, display, page.rgba.len());
                        }
                        Err(error) => {
                            if let Some(tab) =
                                self.tabs.iter_mut().find(|tab| tab.id == key.document_id)
                            {
                                tab.state.error = Some(error);
                                tab.state.rendering = false;
                            }
                        }
                    }
                }
                WorkerEvent::Content(id, page, result) => {
                    self.pending_content.remove(&(id, page));
                    if !self.tabs.iter().any(|tab| tab.id == id) {
                        continue;
                    }
                    match result {
                        Ok(content) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id)
                                && tab.select_all_pending
                                && tab.state.document.as_ref().is_some_and(|d| d.page == page)
                            {
                                tab.select_all_pending = false;
                                tab.selection = content
                                    .glyphs
                                    .first()
                                    .zip(content.glyphs.last())
                                    .map(|(first, last)| (page, first.index, last.index));
                            }
                            let bytes = content.memory_bytes();
                            self.content.insert((id, page), content, bytes);
                        }
                        Err(error) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                                tab.state.error = Some(error);
                            }
                        }
                    }
                }
                WorkerEvent::Searched {
                    id,
                    generation,
                    hits,
                    scanned,
                    total,
                } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id)
                        && tab.search.generation == generation
                    {
                        let first = tab.search.hits.is_empty() && !hits.is_empty();
                        tab.search.hits.extend(hits);
                        if tab.search.hits.len() >= crate::document::MAX_SEARCH_HITS {
                            tab.search.hits.truncate(crate::document::MAX_SEARCH_HITS);
                            tab.search.limited = true;
                            if let Some(worker) = &self.worker {
                                let _ = worker.stop_search(id, generation);
                            }
                        }
                        tab.search.scanned = scanned;
                        tab.search.total = total;
                        if first {
                            tab.search.index = 0;
                            tab.go_to(tab.search.hits[0].page, true);
                            tab.reveal = tab.search.hits[0]
                                .bounds
                                .first()
                                .copied()
                                .map(|bounds| (tab.search.hits[0].page, bounds));
                        }
                    }
                }
                WorkerEvent::Changed(id, state) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        tab.local_marks = state.count;
                        if tab.state.content_revision != state.revision {
                            tab.state.content_revision = state.revision;
                            tab.state.requested = None;
                            tab.state.rendering = false;
                            self.pages.retain(|key| key.document_id != id);
                            self.pending_pages.retain(|key| key.document_id != id);
                        }
                    }
                }
                WorkerEvent::Marked(id, result) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                        tab.marking = false;
                    }
                    match result {
                        Ok((page, count)) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) {
                                tab.local_marks = count;
                                tab.selection = None;
                            }
                            self.pages
                                .retain(|key| key.document_id != id || key.page != page);
                            self.pending_pages
                                .retain(|key| key.document_id != id || key.page != page);
                            self.notice = Some(
                                "Marca conservada. Guardar una copia la incluye en el PDF.".into(),
                            );
                        }
                        Err(error) => {
                            self.notice = Some(format!("No se pudo conservar la marca: {error}"))
                        }
                    }
                }
                WorkerEvent::Saved(_, path, result) => match result {
                    Ok(()) => self.notice = Some(format!("Copia guardada: {}", path.display())),
                    Err(error) => self.error(error),
                },
                WorkerEvent::Unavailable(error) => {
                    self.worker = None;
                    for tab in &mut self.tabs {
                        tab.state.opening = None;
                        tab.state.rendering = false;
                    }
                    self.error(error);
                }
            }
        }
    }
    fn need_page(&mut self, key: RenderKey) -> Option<Arc<DisplayPage>> {
        self.frame_keys.push(key);
        if let Some(page) = self.pages.get(&key) {
            return Some(page);
        }
        if self.pending_pages.insert(key)
            && let Some(worker) = &self.worker
            && let Err(error) = worker.render(key)
        {
            self.pending_pages.remove(&key);
            self.error(error);
        }
        None
    }
    fn need_content(&mut self, id: DocumentId, page: usize) -> Option<Arc<PageContent>> {
        // Text is requested only for pages that can receive interaction.
        if let Some(content) = self.content.get(&(id, page)) {
            return Some(content);
        }
        if self.pending_content.insert((id, page))
            && let Some(worker) = &self.worker
            && let Err(error) = worker.content(id, page)
        {
            self.pending_content.remove(&(id, page));
            self.error(error);
        }
        None
    }
    fn start_search(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            self.next_search += 1;
            tab.search.generation = self.next_search;
            tab.search.hits.clear();
            tab.search.index = 0;
            tab.search.scanned = 0;
            tab.search.limited = false;
            tab.reveal = None;
            tab.search.total = tab.state.document.as_ref().map_or(0, |d| d.info.page_count);
            tab.search.due = Some(Instant::now() + Duration::from_millis(200));
        }
    }
    fn prefetch_page(&mut self, key: RenderKey) {
        self.frame_keys.push(key);
        if self.pages.contains(&key) {
            return;
        }
        if self.pending_pages.insert(key)
            && let Some(worker) = &self.worker
            && let Err(error) = worker.prefetch(key)
        {
            self.pending_pages.remove(&key);
            self.error(error);
        }
    }
    fn step_search(&mut self, forward: bool) {
        if let Some(tab) = self.tabs.get_mut(self.active)
            && !tab.search.hits.is_empty()
        {
            let count = tab.search.hits.len();
            tab.search.index = if forward {
                (tab.search.index + 1) % count
            } else {
                (tab.search.index + count - 1) % count
            };
            tab.go_to(tab.search.hits[tab.search.index].page, true);
            let hit = &tab.search.hits[tab.search.index];
            tab.reveal = hit.bounds.first().copied().map(|bounds| (hit.page, bounds));
        }
    }
    fn selected_text(&mut self) -> Option<String> {
        let tab = self.tabs.get(self.active)?;
        let (page, start, end) = tab.selection?;
        Some(self.content.get(&(tab.id, page))?.text(start, end))
    }
    fn enabled(&self, action: Action) -> bool {
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
                self.tabs
                    .get(self.active)
                    .is_some_and(|tab| tab.selection.is_some() && !tab.marking)
                    && self
                        .state()
                        .details
                        .as_ref()
                        .is_some_and(|details| details.can_annotate && details.can_copy)
            }
            Action::UndoMarkup => self
                .tabs
                .get(self.active)
                .is_some_and(|tab| tab.local_marks > 0 && !tab.marking),
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
                    .is_some_and(|t| t.selection.is_some())
                    && self
                        .state()
                        .details
                        .as_ref()
                        .is_some_and(|details| details.can_copy)
            }
            Action::FindNext | Action::FindPrevious => self
                .tabs
                .get(self.active)
                .is_some_and(|t| !t.search.hits.is_empty()),
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
            tab.pending_position = None;
        }
        match action {
            Action::Highlight | Action::Underline => {
                let tab = &mut self.tabs[self.active];
                if let Some((page, start, end)) = tab.selection
                    && let Some(worker) = &self.worker
                {
                    let mark = crate::document::TextMarkup {
                        page,
                        start,
                        end,
                        kind: if action == Action::Highlight {
                            crate::document::MarkupKind::Highlight
                        } else {
                            crate::document::MarkupKind::Underline
                        },
                        color: self.preferences.markup_color,
                    };
                    match worker.mark(tab.id, mark) {
                        Ok(()) => tab.marking = true,
                        Err(error) => self.error(error),
                    }
                }
            }
            Action::UndoMarkup => {
                let tab = &mut self.tabs[self.active];
                if let Some(worker) = &self.worker {
                    match worker.undo_mark(tab.id) {
                        Ok(()) => tab.marking = true,
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
                let keep = self.tabs[self.active].id;
                for index in (0..self.tabs.len()).rev() {
                    if self.tabs[index].id != keep {
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
                let path = self.tabs[self.active].path.clone();
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
            Action::CopyPath => ctx.copy_text(self.tabs[self.active].path.display().to_string()),
            Action::Exit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Action::DuplicateWindow => {
                let path = self.tabs[self.active].path.clone();
                if let Err(e) = platform::new_window_with_paths(Some(&path), &self.app_paths) {
                    self.error(e);
                }
            }
            Action::Reveal => {
                let path = self.tabs[self.active].path.clone();
                if let Err(e) = platform::reveal(&path) {
                    self.error(e);
                }
            }
            Action::Properties => self.properties_open = true,
            Action::Previous => self.navigate(false),
            Action::Next => self.navigate(true),
            Action::First => self.tabs[self.active].state.go_to(0, true),
            Action::Last => {
                let tab = &mut self.tabs[self.active];
                let count = tab.state.document.as_ref().map_or(1, |d| d.info.page_count);
                tab.state.go_to(count - 1, true);
            }
            Action::GoTo => {
                self.go_to_value = self.state().document.as_ref().map_or(1, |d| d.page + 1);
                self.go_to_open = true;
            }
            Action::Back | Action::Forward => self.tabs[self.active]
                .state
                .history_navigate(action == Action::Forward),
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
                let before = tab.state.zoom;
                tab.state.change_zoom(action == Action::ZoomIn, zoom);
                if tab.state.zoom != before {
                    tab.zoom_anchor = anchor;
                    if anchor.is_some() {
                        tab.state.scroll_to_page = false;
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
                .state
                .rotate(action == Action::RotateRight),
            Action::Single | Action::Facing | Action::Book => {
                let tab = &mut self.tabs[self.active];
                tab.state.layout = match action {
                    Action::Facing => LayoutMode::Facing,
                    Action::Book => LayoutMode::Book,
                    _ => LayoutMode::Single,
                };
                tab.state.invalidate();
                tab.state.scroll_to_page = true;
            }
            Action::Continuous => {
                let tab = &mut self.tabs[self.active];
                tab.state.continuous = !tab.state.continuous;
                tab.state.invalidate();
                tab.state.scroll_to_page = true;
            }
            Action::Manga => {
                self.tabs[self.active].state.manga = !self.tabs[self.active].state.manga
            }
            Action::VerticalReading => {
                let tab = &mut self.tabs[self.active];
                tab.state.layout = LayoutMode::Single;
                tab.state.continuous = true;
                tab.state.set_zoom(Zoom::FitWidth);
                tab.state.invalidate();
                tab.state.scroll_to_page = true;
                tab.scroll.x = 0.0;
            }
            Action::Find => {
                let tab = &mut self.tabs[self.active];
                tab.search.open = true;
                tab.search.focus = true;
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
                tab.select_all_pending = true;
                let id = tab.id;
                let page = tab.state.document.as_ref().map_or(0, |d| d.page);
                if let Some(content) = self.need_content(id, page) {
                    let tab = &mut self.tabs[self.active];
                    tab.select_all_pending = false;
                    tab.selection = content
                        .glyphs
                        .first()
                        .zip(content.glyphs.last())
                        .map(|(a, b)| (page, a.index, b.index));
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
                if let Some(saved) = self.tabs[self.active].stored() {
                    if let Some(index) = self
                        .preferences
                        .favorites
                        .iter()
                        .position(|f| f.path == saved.path && f.page == saved.page)
                    {
                        self.preferences.favorites.remove(index);
                    } else {
                        let name = saved.path.file_name().unwrap_or_default().to_string_lossy();
                        self.preferences.favorites.push(Favorite {
                            path: saved.path.clone(),
                            page: saved.page,
                            label: format!("{} · {}", name, saved.page + 1),
                        });
                    }
                    self.save_preferences();
                }
            }
            Action::Theme => self.toggle_theme(ctx),
            Action::Invert | Action::Grayscale => {
                let tab = &mut self.tabs[self.active];
                let selected = if action == Action::Invert {
                    crate::document::PageColors::Inverted
                } else {
                    crate::document::PageColors::Grayscale
                };
                tab.state.colors = if tab.state.colors == selected {
                    crate::document::PageColors::Original
                } else {
                    selected
                };
                tab.state.invalidate();
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
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            Action::Presentation => {
                if self.presentation {
                    self.leave_presentation(ctx);
                } else {
                    self.presentation_previous_fullscreen = Some(self.fullscreen);
                    self.presentation = true;
                    self.fullscreen = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
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
    fn effective_zoom(&mut self) -> f32 {
        match self.state().zoom {
            Zoom::Percent(p) => f32::from(p),
            _ => self
                .state()
                .requested
                .and_then(|key| self.pages.get(&key))
                .map_or(100.0, |page| page.zoom),
        }
    }
    fn leave_presentation(&mut self, ctx: &egui::Context) {
        self.presentation = false;
        self.fullscreen = self
            .presentation_previous_fullscreen
            .take()
            .unwrap_or(false);
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }
    fn scroll(&mut self, action: Action) {
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
        let fits_horizontally = tab.view_layout.as_ref().is_some_and(|(_, layout)| {
            let Some(document) = &tab.state.document else {
                return false;
            };
            let spread = tab
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
            left.is_finite() && right - left <= tab.viewport.x + 1.0
        });
        if horizontal
            && (matches!(tab.state.zoom, Zoom::FitWidth | Zoom::FitPage) || fits_horizontally)
        {
            self.navigate(forward);
            return;
        }
        let screen = if horizontal {
            tab.viewport.x
        } else {
            tab.viewport.y
        };
        let step = match action {
            Action::ScrollUpPage
            | Action::ScrollDownPage
            | Action::ScrollLeftPage
            | Action::ScrollRightPage => screen * 0.9,
            Action::ScrollUpHalf | Action::ScrollDownHalf => screen * 0.5,
            _ => 40.0,
        };
        let content = tab.view_layout.as_ref().map_or(screen, |(_, layout)| {
            if horizontal {
                layout.width
            } else {
                layout.height
            }
        });
        let offset = if horizontal {
            &mut tab.scroll.x
        } else {
            &mut tab.scroll.y
        };
        let old = *offset;
        *offset =
            (*offset + if forward { step } else { -step }).clamp(0.0, (content - screen).max(0.0));
        if (*offset - old).abs() < 0.1 && (horizontal || !tab.state.continuous) {
            self.navigate(forward);
            if !forward && !horizontal {
                self.tabs[self.active].scroll_bottom = true;
            }
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            let dismissing = self.palette_open
                || self.go_to_open
                || self.zoom_open
                || self.properties_open
                || self.settings_open
                || !self.home
                    && self
                        .tabs
                        .get(self.active)
                        .is_some_and(|tab| tab.password_needed || tab.search.open);
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
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            }
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.selection = None;
                tab.search.open = false;
                tab.password_needed = false;
                tab.password.clear();
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
                .is_some_and(|tab| tab.password_needed)
            || egui::Popup::is_any_open(ctx);
        let (wheel_zoom, pointer) =
            ctx.input(|input| (input.zoom_delta(), input.pointer.hover_pos()));
        if !keyboard
            && self.state().document.is_some()
            && wheel_zoom != 1.0
            && let Some(tab) = self.tabs.get(self.active)
            && pointer
                .zip(tab.viewport_rect)
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
            (egui::Modifiers::NONE, egui::Key::F8, Action::Toolbar),
            (egui::Modifiers::NONE, egui::Key::F9, Action::MenuBar),
            (egui::Modifiers::NONE, egui::Key::F12, Action::ToggleSidebar),
        ];
        for (modifiers, key, action) in bindings {
            if consume_reader_key(ctx, modifiers, key) {
                self.execute(action, ctx);
            }
        }
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
impl eframe::App for ReaderApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(writer) = &self.preference_writer {
            while let Some(result) = writer.poll() {
                self.config_warning = result.err();
            }
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
            && let Some(due) = tab.search.due
        {
            if due <= Instant::now() {
                tab.search.due = None;
                if let Some(worker) = &self.worker
                    && let Err(error) = worker.search(SearchRequest {
                        document_id: tab.id,
                        generation: tab.search.generation,
                        query: tab.search.query.clone(),
                        match_case: tab.search.match_case,
                        whole_word: tab.search.whole_word,
                    })
                {
                    tab.state.error = Some(error);
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
        if let Some(writer) = &mut self.preference_writer {
            self.config_warning = writer.finish().err();
        }
    }
}
