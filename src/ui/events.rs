//! Desktop events coordination. Reading rules live in crabpdf-application.
use super::*;

impl ReaderApp {
    pub(super) fn error(&mut self, error: ReaderError) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.reader.state.error = Some(error);
        } else {
            self.empty.error = Some(error);
        }
    }
    pub(super) fn schedule_save(&mut self) {
        self.save_due = Some(Instant::now() + Duration::from_millis(500));
    }
    pub(super) fn save_preferences(&mut self) {
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
        self.config_warning = self.profile.request(self.preferences.clone()).err();
        self.save_due = None;
    }
    pub(super) fn open_dialog(&mut self) {
        if self.dialog.is_none() && self.worker.is_some() {
            self.dialog = Some(crate::desktop::dialogs::PendingDialog::open());
        }
    }
    pub(super) fn save_dialog(&mut self) {
        if self.dialog.is_some() {
            return;
        }
        if let Some(tab) = self.tabs.get(self.active) {
            let name = tab.reader.path.with_extension("pdf");
            let name = name
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            self.dialog = Some(crate::desktop::dialogs::PendingDialog::save(
                tab.reader.id,
                name,
            ));
        }
    }
    pub(super) fn poll_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.as_mut() else {
            return;
        };
        if let Some((kind, files)) = dialog.poll(ctx) {
            self.dialog = None;
            for path in files {
                if let Some(id) = kind {
                    if let Some(worker) = &self.worker
                        && let Err(error) = worker.save(id, path)
                    {
                        self.error(error);
                    }
                } else {
                    self.open_path(path);
                }
            }
        }
    }

    pub(super) fn poll_worker(&mut self, ctx: &egui::Context) {
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
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id) {
                        let supports_password = tab.supports_password();
                        if let crabpdf_application::tabs::OpenOutcome::Ready { initial_search } =
                            tab.reader.finish_open(id, result, supports_password)
                        {
                            if initial_search {
                                self.next_search += 1;
                                tab.reader.restart_search(self.next_search);
                                tab.view.search.open = true;
                                tab.view.search.due =
                                    tab.reader.search.is_active().then(Instant::now);
                            }
                            if let Some(saved) = tab.stored() {
                                self.preferences.remember(saved);
                                self.preferences.record_open(&tab.reader.path);
                            }
                            self.schedule_save();
                        }
                    }
                }
                WorkerEvent::Details(id, result) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id) {
                        match result {
                            Ok(details) => {
                                if let Some(warning) = tab
                                    .reader
                                    .apply_details(details, &mut self.preferences.library)
                                {
                                    self.notice = Some(warning);
                                }
                            }
                            Err(error) => tab.reader.state.error = Some(error),
                        }
                    }
                }
                WorkerEvent::Rendered(key, result) => {
                    self.pending_pages.remove(&key);
                    // Also release stale results and failures: only the display
                    // cache needs to retain pixels after this event is consumed.
                    if let Some(worker) = &self.worker {
                        let _ = worker.release_render(key);
                    }
                    if !self.tabs.iter().any(|tab| {
                        tab.reader.id == key.document_id
                            && tab.reader.state.content_revision == key.revision
                    }) {
                        continue;
                    }
                    match result {
                        Ok(page) => {
                            if let Some(tab) = self
                                .tabs
                                .iter_mut()
                                .find(|tab| tab.reader.id == key.document_id)
                            {
                                if let Some(size) = tab.reader.state.page_sizes.get_mut(key.page)
                                    && *size != Some(page.page_size)
                                {
                                    *size = Some(page.page_size);
                                    tab.reader.layout_revision += 1;
                                }
                                if tab.reader.state.requested == Some(key) {
                                    tab.reader.state.rendering = false;
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
                                limited: page.resolution_limited,
                            });
                            self.pages.insert(key, display, page.rgba.len());
                        }
                        Err(error) => {
                            if let Some(tab) = self
                                .tabs
                                .iter_mut()
                                .find(|tab| tab.reader.id == key.document_id)
                            {
                                tab.reader.state.error = Some(error);
                                tab.reader.state.rendering = false;
                            }
                        }
                    }
                }
                WorkerEvent::Content(id, page, result) => {
                    self.pending_content.remove(&(id, page));
                    if !self.tabs.iter().any(|tab| tab.reader.id == id) {
                        continue;
                    }
                    match result {
                        Ok(content) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id)
                            {
                                tab.reader.accept_content(id, page, &content);
                            }
                            let bytes = content.memory_bytes();
                            self.content.insert((id, page), content, bytes);
                        }
                        Err(error) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id)
                            {
                                tab.reader.selection.content_failed(id, page);
                                tab.reader.state.error = Some(error);
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
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id) {
                        let update = tab
                            .reader
                            .search
                            .accept(id, generation, hits, scanned, total);
                        if let Some(stop) = update.stop
                            && let Some(worker) = &self.worker
                        {
                            let _ = worker.stop_search(stop.document_id, stop.generation);
                        }
                        if let Some(destination) = update.destination {
                            tab.reveal_search(destination);
                        }
                    }
                }
                WorkerEvent::SearchFailed {
                    id,
                    generation,
                    error,
                } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id) {
                        tab.reader.search.fail(id, generation, error);
                    }
                }
                WorkerEvent::Changed(id, state) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id) {
                        tab.reader.local_marks = state.count;
                        if tab.reader.state.content_revision != state.revision {
                            tab.reader.state.content_revision = state.revision;
                            tab.reader.state.requested = None;
                            tab.reader.state.rendering = false;
                            // Keep bounded display snapshots until a replacement arrives.
                            // They are never accepted as a current render or engine input.
                            self.pending_pages.retain(|key| key.document_id != id);
                        }
                    }
                }
                WorkerEvent::Marked(id, result) => {
                    if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id) {
                        tab.reader.marking = false;
                    }
                    match result {
                        Ok((_page, count)) => {
                            if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.reader.id == id)
                            {
                                tab.reader.local_marks = count;
                                tab.reader.selection.clear();
                            }
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
                        tab.reader.state.opening = None;
                        tab.reader.state.rendering = false;
                    }
                    self.error(error);
                }
            }
        }
    }
}
