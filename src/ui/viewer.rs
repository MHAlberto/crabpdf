use super::{Action, ReaderApp, theme};
use crate::document::{Bounds, LinkTarget, PageContent, PageSize};
use crate::layout::{LayoutSpec, PageLayout, visible_tiles};
use eframe::egui::{self, Color32, Rect, Vec2, emath::GuiRounding};
use std::sync::Arc;

impl ReaderApp {
    /// Keep the last view briefly on a cold jump; switch only to a preview
    /// belonging to the requested document/content revision. Long jumps snap,
    /// nearby continuous jumps scroll, and no two page texts are crossfaded.
    fn prepare_page_navigation(
        &mut self,
        ui: &egui::Ui,
        page: usize,
        width: u32,
        height: u32,
        now: f64,
    ) -> bool {
        let tab = &self.tabs[self.active];
        if !tab.reader.state.scroll_to_page || tab.view.displayed_page.is_none_or(|old| old == page)
        {
            self.tabs[self.active].view.navigation_wait = None;
            return false;
        }
        let (cell_width, cell_height) = tab
            .view
            .view_layout
            .as_ref()
            .map_or((width as f32, height as f32), |(spec, _)| spec.cell_size());
        let Some(key) = tab.reader.state.render_key_for(
            page,
            cell_width,
            cell_height,
            ui.ctx().pixels_per_point(),
            ui.input(|input| input.max_texture_side),
        ) else {
            return false;
        };
        let count = tab
            .reader
            .state
            .document
            .as_ref()
            .map_or(0, |document| document.info.page_count);
        let spread = tab.reader.state.layout.spread(page, count);
        let mut ready = true;
        for page in spread {
            let found = self
                .pages
                .most_recent(|cached| {
                    cached.document_id == key.document_id
                        && cached.revision == key.revision
                        && cached.page == page
                        && cached.rotation == key.rotation
                        && cached.colors == key.colors
                        && cached.region.is_none()
                })
                .is_some();
            if !found {
                self.need_page(crate::document::RenderKey {
                    page,
                    bitmap_side_limit: key.bitmap_side_limit.min(768),
                    region: None,
                    ..key
                });
            }
            ready &= found;
        }
        let tab = &mut self.tabs[self.active];
        if tab
            .view
            .navigation_wait
            .is_none_or(|(target, _)| target != page)
        {
            tab.view.navigation_wait = Some((page, now));
        }
        let waiting = !ready
            && tab.reader.state.error.is_none()
            && tab
                .view
                .navigation_wait
                .is_some_and(|(_, started)| now - started < 0.2);
        if waiting {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(16));
        } else {
            tab.view.navigation_wait = None;
        }
        waiting
    }
    pub(super) fn document_view(&mut self, ui: &mut egui::Ui) {
        let canvas = if self.presentation {
            Color32::BLACK
        } else {
            theme::palette(self.preferences.theme).canvas
        };
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(canvas))
            .show(ui, |ui| {
                let identity = if self.home {
                    (0, false)
                } else {
                    self.tabs.get(self.active).map_or((0, false), |tab| {
                        (tab.reader.id, tab.reader.state.opening.is_some())
                    })
                };
                self.entrance
                    .apply(ui, identity, self.preferences.reduce_motion);
                if self.state().opening.is_some() {
                    ui.centered_and_justified(|ui| {
                        ui.label("Abriendo documento…");
                    });
                    return;
                }
                if self.state().document.is_none() {
                    self.library_home(ui);
                    return;
                }
                let compact = self.fullscreen;
                // Reserving animated scrollbar widths changes FitWidth and schedules
                // another raster. Keep the reading viewport stable in every mode.
                ui.spacing_mut().scroll = egui::style::ScrollStyle::floating();
                ui.spacing_mut().scroll.fade.strength = 0.0;
                let width = ui.available_width().round().max(1.0) as u32;
                let height = ui.available_height().round().max(1.0) as u32;
                let now = ui.input(|input| input.time);
                {
                    let tab = &mut self.tabs[self.active];
                    if ui.input(|input| {
                        input.smooth_scroll_delta != Vec2::ZERO
                            || (input.pointer.any_pressed()
                                && tab.view.viewport_rect.is_some_and(|rect| {
                                    input
                                        .pointer
                                        .interact_pos()
                                        .is_some_and(|pos| rect.contains(pos))
                                }))
                    }) {
                        tab.reader.pending_position = None;
                        if (tab.reader.state.scroll_to_page || tab.view.scroll_motion.is_some())
                            && let Some(displayed) = tab.view.displayed_page
                            && let Some(document) = &mut tab.reader.state.document
                        {
                            document.page = displayed;
                            tab.reader.selection.page_changed(displayed);
                        }
                        tab.view.scroll_motion = None;
                        tab.view.key_scroll.cancel();
                        tab.view.navigation_wait = None;
                        tab.reader.state.scroll_to_page = false;
                    }
                }
                let current = self.state().document.as_ref().map_or(0, |d| d.page);
                let waiting = self.prepare_page_navigation(ui, current, width, height, now);
                let tab = &mut self.tabs[self.active];
                let animate_navigation = !self.preferences.reduce_motion
                    && tab.reader.state.continuous
                    && tab.view.view_layout.as_ref().is_some_and(|(old, _)| {
                        old.continuous
                            && old.zoom == tab.reader.state.zoom
                            && old.mode == tab.reader.state.layout
                            && old.rotation == tab.reader.state.rotation
                            && old.width == width
                            && old.height == height
                    });
                let mut reset_scroll = false;
                let spec = LayoutSpec {
                    width,
                    height,
                    zoom: tab.reader.state.zoom,
                    mode: tab.reader.state.layout,
                    continuous: tab.reader.state.continuous,
                    current: if tab.reader.state.continuous {
                        0
                    } else if waiting {
                        tab.view.displayed_page.unwrap_or(current)
                    } else {
                        current
                    },
                    rotation: tab.reader.state.rotation,
                    manga: tab.reader.state.manga,
                    compact,
                    revision: tab.reader.layout_revision,
                };
                if tab
                    .view
                    .view_layout
                    .as_ref()
                    .is_none_or(|(old, _)| *old != spec)
                {
                    let resize_anchor = tab
                        .view
                        .view_layout
                        .as_ref()
                        .filter(|(old, _)| old.width != spec.width || old.height != spec.height)
                        .and_then(|_| {
                            let mut anchor = tab.anchor(None)?;
                            anchor.screen_x = width as f32 * 0.5;
                            anchor.screen_y = height as f32 * 0.5;
                            Some(anchor)
                        });
                    let previous_position =
                        tab.view.view_layout.as_ref().and_then(|(_, layout)| {
                            layout
                                .placements
                                .iter()
                                .find(|p| p.page == current)
                                .map(|p| (p.left, p.top))
                        });
                    let fallback = tab
                        .reader
                        .state
                        .details
                        .as_ref()
                        .map_or(PageSize::default(), |d| d.first_page_size);
                    let layout = Arc::new(PageLayout::build(
                        &tab.reader.state.page_sizes,
                        fallback,
                        spec,
                    ));
                    if tab.view.zoom_anchor.is_none() && !tab.reader.state.scroll_to_page {
                        tab.view.zoom_anchor = resize_anchor;
                    }
                    if !tab.reader.state.scroll_to_page
                        && let Some((old_left, old_top)) = previous_position
                        && let Some(placement) =
                            layout.placements.iter().find(|p| p.page == current)
                    {
                        let delta = egui::vec2(placement.left - old_left, placement.top - old_top);
                        tab.view.scroll = (tab.view.scroll + delta).max(Vec2::ZERO);
                        tab.view.key_scroll.shift(delta.y);
                        if let Some(motion) = &mut tab.view.scroll_motion {
                            motion.shift(delta);
                        }
                    }
                    tab.view.view_layout = Some((spec, layout));
                }
                let layout = Arc::clone(
                    &tab.view
                        .view_layout
                        .as_ref()
                        .expect("layout was constructed")
                        .1,
                );
                if let Some(position) = tab.reader.pending_position
                    && tab
                        .reader
                        .state
                        .page_sizes
                        .get(position.page())
                        .is_some_and(Option::is_some)
                    && let Some(anchor) = position.anchor(
                        (width as f32, height as f32),
                        tab.reader.state.page_sizes.len(),
                    )
                {
                    tab.reader.pending_position = None;
                    tab.view.zoom_anchor = Some(anchor);
                    tab.reader.state.scroll_to_page = false;
                }
                if tab.reader.state.scroll_to_page {
                    tab.view.key_scroll.cancel();
                    tab.view.zoom_anchor = None;
                    tab.view.scroll_motion = None;
                }
                if let Some(anchor) = tab.view.zoom_anchor.take()
                    && let Some((x, y)) = anchor.offset(&layout)
                {
                    tab.view.scroll = egui::vec2(x, y);
                    tab.view.key_scroll.cancel();
                    tab.view.scroll_motion = None;
                    tab.reader.state.scroll_to_page = false;
                    reset_scroll = true;
                }
                if tab.reader.state.scroll_to_page && !waiting {
                    let target = layout
                        .placements
                        .iter()
                        .find(|p| p.page == current)
                        .map_or(Vec2::ZERO, |p| {
                            egui::vec2(
                                (p.left - spec.gutter()).max(0.0),
                                (p.top - spec.gutter()).max(0.0),
                            )
                        })
                        .min(egui::vec2(
                            (layout.width - width as f32).max(0.0),
                            (layout.height - height as f32).max(0.0),
                        ));
                    let distance = (target - tab.view.scroll).length();
                    if animate_navigation && distance > 1.0 && distance <= height as f32 * 2.5 {
                        tab.view.scroll_motion = Some(super::motion::ScrollMotion::new(
                            tab.view.scroll,
                            target,
                            now,
                        ));
                    } else {
                        tab.view.scroll = target;
                    }
                    tab.reader.state.scroll_to_page = false;
                    reset_scroll = true;
                }
                if tab.view.scroll_bottom {
                    tab.view.scroll.y = (layout.height - tab.view.viewport.y).max(0.0);
                    tab.view.scroll_bottom = false;
                }
                if tab.view.reveal.is_some_and(|(page, _)| page != current) {
                    tab.view.reveal = None;
                }
                if let Some((page, bounds)) = tab.view.reveal
                    && tab
                        .reader
                        .state
                        .page_sizes
                        .get(page)
                        .is_some_and(Option::is_some)
                    && let Some(placement) = layout.placements.iter().find(|p| p.page == page)
                {
                    tab.view.reveal = None;
                    tab.view.scroll_motion = None;
                    reset_scroll = true;
                    let bounds = bounds.rotated(spec.rotation);
                    tab.view.scroll.x = (placement.left
                        + (bounds.left + bounds.right) * 0.5 * placement.size.width
                        - width as f32 * 0.5)
                        .max(0.0);
                    tab.view.scroll.y = (placement.top
                        + (bounds.top + bounds.bottom) * 0.5 * placement.size.height
                        - height as f32 * 0.5)
                        .max(0.0);
                }
                if let Some(motion) = tab.view.scroll_motion {
                    let (offset, active) = motion.sample(if self.preferences.reduce_motion {
                        now + f64::from(super::motion::DURATION)
                    } else {
                        now
                    });
                    tab.view.scroll = offset;
                    if active {
                        ui.ctx().request_repaint();
                    } else {
                        tab.view.scroll_motion = None;
                    }
                }
                if let Some(offset) = tab.view.key_scroll.advance(
                    tab.view.scroll.y,
                    (layout.height - height as f32).max(0.0),
                    ui.input(|input| input.stable_dt),
                    self.preferences.reduce_motion,
                ) {
                    tab.view.scroll.y = offset;
                    reset_scroll = true;
                    if tab.view.key_scroll.active() {
                        ui.ctx().request_repaint();
                    }
                }
                let id = tab.reader.id;
                let scroll = tab.view.scroll;
                if reset_scroll {
                    // Clear egui's previous kinetic/scroll-to target. Otherwise it
                    // can pull the newly requested page back on the following frame.
                    let mut state = egui::scroll_area::State::default();
                    state.offset = scroll;
                    state.store(ui.ctx(), ui.make_persistent_id(("pdf-scroll", id)));
                }
                let mut pan = Vec2::ZERO;
                let mut visible_current = current;
                let mut best_visible = 0.0;
                let mut displayed_keys = Vec::new();
                let density = ui.ctx().pixels_per_point();
                let texture_limit = ui.input(|i| i.max_texture_side);
                let (cell_width, cell_height) = spec.cell_size();
                let vertical_only = matches!(
                    spec.zoom,
                    crate::document::Zoom::FitWidth | crate::document::Zoom::FitPage
                );
                let area = if vertical_only {
                    egui::ScrollArea::vertical()
                } else {
                    egui::ScrollArea::both()
                };
                let output = area
                    .id_salt(("pdf-scroll", id))
                    .animated(false)
                    .auto_shrink([false, false])
                    .scroll_offset(scroll)
                    .show_viewport(ui, |ui, viewport| {
                        let origin = ui.min_rect().min;
                        ui.set_min_size(egui::vec2(layout.width, layout.height));
                        for placement in &layout.placements {
                            let relative = Rect::from_min_size(
                                egui::pos2(placement.left, placement.top),
                                egui::vec2(placement.size.width, placement.size.height),
                            );
                            if !relative.intersects(viewport.expand(height as f32))
                                && placement.page.abs_diff(current) > 1
                            {
                                continue;
                            }
                            let visible = relative.intersect(viewport).area().max(0.0);
                            if visible > best_visible {
                                best_visible = visible;
                                visible_current = placement.page;
                            }
                            let rect = relative
                                .translate(origin.to_vec2())
                                .round_to_pixels(density);
                            let Some(key) = self.state().render_key_for(
                                placement.page,
                                cell_width,
                                cell_height,
                                density,
                                texture_limit,
                            ) else {
                                continue;
                            };
                            // A bounded full-page preview is prepared for both neighbors.
                            // It stays visible while sharper tiles are still arriving.
                            let preview_key = crate::document::RenderKey {
                                bitmap_side_limit: key.bitmap_side_limit.min(768),
                                region: None,
                                ..key
                            };
                            if !relative.intersects(viewport.expand(80.0)) {
                                self.prefetch_page(preview_key);
                                continue;
                            }
                            ui.painter().rect_filled(rect, 0, Color32::WHITE);
                            let preview = super::raster::preview(&mut self.pages, key);
                            let preview_revision = preview.as_ref().map(|(revision, _)| *revision);
                            if preview_revision != Some(key.revision) {
                                // Refresh the visible overview before expensive sharp tiles.
                                self.need_page(preview_key);
                            } else {
                                self.prefetch_page(preview_key);
                            }
                            if let Some((_, preview)) = preview {
                                ui.painter().image(
                                    preview.texture.id(),
                                    rect,
                                    Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            let pixel_width = (rect.width() * key.density_milli as f32 / 1000.0)
                                .round()
                                .max(1.0) as u32;
                            let pixel_height = (rect.height() * key.density_milli as f32 / 1000.0)
                                .round()
                                .max(1.0) as u32;
                            let known_size = self
                                .state()
                                .page_sizes
                                .get(placement.page)
                                .is_some_and(Option::is_some);
                            let mut regions = vec![None];
                            if known_size && (pixel_width > 1024 || pixel_height > 1024) {
                                let visible = relative.intersect(viewport.expand(80.0));
                                regions = visible_tiles(
                                    pixel_width,
                                    pixel_height,
                                    Bounds {
                                        left: (visible.left() - relative.left()) / relative.width(),
                                        top: (visible.top() - relative.top()) / relative.height(),
                                        right: (visible.right() - relative.left())
                                            / relative.width(),
                                        bottom: (visible.bottom() - relative.top())
                                            / relative.height(),
                                    },
                                    key.bitmap_side_limit,
                                )
                                .into_iter()
                                .map(Some)
                                .collect();
                            }
                            for region in regions {
                                let key = crate::document::RenderKey { region, ..key };
                                displayed_keys.push(key);
                                if let Some(display) = self.need_page(key).or_else(|| {
                                    super::raster::stale_tile(
                                        &mut self.pages,
                                        key,
                                        preview_revision,
                                    )
                                }) {
                                    let target = if let Some(region) = region {
                                        let scale = egui::vec2(
                                            rect.width() / pixel_width as f32,
                                            rect.height() / pixel_height as f32,
                                        );
                                        Rect::from_min_size(
                                            rect.min
                                                + egui::vec2(
                                                    region.x as f32 * scale.x,
                                                    region.y as f32 * scale.y,
                                                ),
                                            egui::vec2(
                                                region.width as f32 * scale.x,
                                                region.height as f32 * scale.y,
                                            ),
                                        )
                                    } else {
                                        rect
                                    };
                                    ui.painter().image(
                                        display.texture.id(),
                                        target.round_to_pixels(density),
                                        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                                        Color32::WHITE,
                                    );
                                }
                            }
                            let response = ui.interact(
                                rect,
                                egui::Id::new(("page", id, placement.page)),
                                egui::Sense::click_and_drag(),
                            );
                            if response.dragged_by(egui::PointerButton::Secondary) {
                                pan -= ui.input(|i| i.pointer.delta());
                            }
                            if let Some(content) = self.need_content(id, placement.page) {
                                self.page_interaction(
                                    ui,
                                    &response,
                                    rect,
                                    placement.page,
                                    &content,
                                    spec.rotation,
                                );
                            }
                            response.context_menu(|ui| {
                                for action in
                                    [Action::Highlight, Action::Underline, Action::UndoMarkup]
                                {
                                    self.menu_action(ui, action);
                                }
                                ui.menu_button("Color de las marcas", |ui| self.markup_colors(ui));
                                ui.separator();
                                for action in [
                                    Action::Copy,
                                    Action::SelectAll,
                                    Action::Find,
                                    Action::RotateLeft,
                                    Action::RotateRight,
                                    Action::ToggleFavorite,
                                    Action::Properties,
                                    Action::SaveAs,
                                    Action::Fullscreen,
                                ] {
                                    self.menu_action(ui, action);
                                }
                            });
                        }
                    });
                // Warm the next/previous spread even in non-continuous mode.
                // Only bounded previews are retained by the existing byte-budgeted cache.
                let count = self
                    .state()
                    .document
                    .as_ref()
                    .map_or(0, |doc| doc.info.page_count);
                let spread = self.state().layout.spread(current, count);
                if let (Some(first), Some(last)) = (spread.first(), spread.last()) {
                    for neighbor in [first.saturating_sub(1), last.saturating_add(1)] {
                        if neighbor >= count || spread.contains(&neighbor) {
                            continue;
                        }
                        for page in self.state().layout.spread(neighbor, count) {
                            if let Some(key) = self.state().render_key_for(
                                page,
                                cell_width,
                                cell_height,
                                density,
                                texture_limit,
                            ) {
                                self.prefetch_page(crate::document::RenderKey {
                                    bitmap_side_limit: key.bitmap_side_limit.min(768),
                                    region: None,
                                    ..key
                                });
                            }
                        }
                    }
                }
                if let Some(tab) = self.tabs.get_mut(self.active)
                    && tab.reader.id == id
                {
                    let previous_scroll = tab.view.scroll;
                    tab.view.scroll = output.state.offset + pan;
                    tab.view.viewport = output.inner_rect.size();
                    tab.view.viewport_rect = Some(output.inner_rect);
                    tab.view.scroll.x = tab.view.scroll.x.max(0.0);
                    if vertical_only {
                        tab.view.scroll.x = 0.0;
                    }
                    tab.view.scroll.y = tab.view.scroll.y.max(0.0);
                    if tab.reader.state.continuous
                        && tab.view.scroll_motion.is_none()
                        && !tab.reader.state.scroll_to_page
                        && let Some(document) = tab.reader.state.document.as_mut()
                    {
                        document.page = visible_current;
                        tab.reader.selection.page_changed(visible_current);
                    }
                    if !waiting {
                        tab.view.displayed_page = Some(visible_current);
                    }
                    let page = tab.reader.state.document.as_ref().map_or(0, |d| d.page);
                    tab.reader.state.requested =
                        displayed_keys.iter().copied().find(|key| key.page == page);
                    tab.reader.state.rendering = displayed_keys
                        .iter()
                        .any(|key| self.pending_pages.contains(key));
                    if (tab.view.scroll - previous_scroll).length_sq() > 0.25 {
                        self.schedule_save();
                    }
                }
            });
    }

    fn page_interaction(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        rect: Rect,
        page: usize,
        content: &PageContent,
        rotation: u8,
    ) {
        // A page is a pointer target, not a text editor. Keeping its widget focus
        // would suppress copy, zoom and annotation shortcuts after selecting words.
        response.surrender_focus();
        let normalize = |position: egui::Pos2| {
            let x = ((position.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            let y = ((position.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
            match rotation % 4 {
                1 => (y, 1.0 - x),
                2 => (1.0 - x, 1.0 - y),
                3 => (1.0 - y, x),
                _ => (x, y),
            }
        };
        if let Some(position) = response.interact_pointer_pos() {
            let (x, y) = normalize(position);
            if response.double_clicked()
                && let Some(index) = content.nearest_glyph(x, y)
            {
                self.tabs[self.active]
                    .reader
                    .selection
                    .word(page, content, index);
            } else if response.drag_started_by(egui::PointerButton::Primary) {
                let (x, y) = normalize(ui.input(|i| i.pointer.press_origin()).unwrap_or(position));
                if let Some(index) = content.nearest_glyph(x, y) {
                    self.tabs[self.active]
                        .reader
                        .selection
                        .select(page, index, index);
                }
            }
            if response.dragged_by(egui::PointerButton::Primary)
                && ui.input(|input| input.pointer.delta() != Vec2::ZERO)
                && let Some(index) = content.nearest_glyph(x, y)
            {
                self.tabs[self.active].reader.selection.extend(page, index);
            }
            if response.clicked() && !response.double_clicked() {
                self.tabs[self.active].reader.selection.clear();
                if let Some(link) = content.links.iter().find(|link| link.bounds.contains(x, y)) {
                    match &link.target {
                        LinkTarget::Page(destination) => {
                            self.tabs[self.active].go_to(*destination, true)
                        }
                        LinkTarget::Uri(uri) => {
                            if uri.starts_with("https://")
                                || uri.starts_with("http://")
                                || uri.starts_with("mailto:")
                            {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(uri));
                            } else {
                                self.notice=Some("Este enlace utiliza un protocolo que el lector todavía no abre.".into());
                            }
                        }
                    }
                }
            }
        }
        let paint_bounds = |bounds: Bounds, color: Color32| {
            let bounds = bounds.rotated(rotation);
            let highlight = Rect::from_min_max(
                egui::pos2(
                    rect.left() + bounds.left * rect.width(),
                    rect.top() + bounds.top * rect.height(),
                ),
                egui::pos2(
                    rect.left() + bounds.right * rect.width(),
                    rect.top() + bounds.bottom * rect.height(),
                ),
            );
            ui.painter().rect_filled(highlight, 0, color);
        };
        if let Some(selection) = self.tabs[self.active].reader.selection.range()
            && selection.page == page
        {
            for bounds in self.tabs[self.active]
                .view
                .selection_geometry
                .get(selection, content)
            {
                paint_bounds(*bounds, Color32::from_rgba_unmultiplied(76, 136, 230, 85));
            }
        }
        let search = &self.tabs[self.active].reader.search;
        for (index, hit) in search
            .hits
            .iter()
            .enumerate()
            .filter(|(_, hit)| hit.page == page)
        {
            for bounds in &hit.bounds {
                paint_bounds(
                    *bounds,
                    if index == search.index {
                        Color32::from_rgba_unmultiplied(240, 170, 20, 120)
                    } else {
                        Color32::from_rgba_unmultiplied(255, 215, 45, 65)
                    },
                );
            }
        }
        if response.hovered()
            && let Some(position) = ui.input(|i| i.pointer.hover_pos())
        {
            let (x, y) = normalize(position);
            if let Some(link) = content.links.iter().find(|link| link.bounds.contains(x, y)) {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                response.clone().on_hover_text(match &link.target {
                    LinkTarget::Page(page) => format!("Ir a página {}", page + 1),
                    LinkTarget::Uri(uri) => uri.clone(),
                });
            } else if !content.glyphs.is_empty() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
            }
        }
    }
}
