use super::*;

use crabpdf_application::profile::ReadingTotals;
use std::time::{Duration, Instant};

const SAMPLE: Duration = Duration::from_secs(2);
const SAVE_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Scope {
    #[default]
    Document,
    Session,
    History,
}

#[derive(Default)]
pub(super) struct Statistics {
    pub(super) current: Option<(DocumentId, usize)>,
    previous: Option<(DocumentId, Instant)>,
    last_page: Option<(DocumentId, usize)>,
    by_document: BTreeMap<DocumentId, ReadingTotals>,
    session: ReadingTotals,
    save_at: Option<Instant>,
    scope: Scope,
}

impl Statistics {
    pub(super) fn sample(
        &mut self,
        ctx: &egui::Context,
        current: Option<(DocumentId, usize)>,
        focused: bool,
        focus_sessions: u64,
        totals: &mut ReadingTotals,
    ) -> bool {
        let now = Instant::now();
        self.current = current;
        self.record(now, current.filter(|_| focused), focus_sessions, totals);
        if current.is_some() && focused {
            ctx.request_repaint_after(SAMPLE);
        }
        if let Some(due) = self.save_at {
            if due <= now {
                self.save_at = None;
                return true;
            }
            ctx.request_repaint_after(due - now);
        }
        false
    }

    fn record(
        &mut self,
        now: Instant,
        active: Option<(DocumentId, usize)>,
        focus_sessions: u64,
        totals: &mut ReadingTotals,
    ) {
        let previous = self.previous.take();
        let mut changed = focus_sessions > 0;
        if let Some((id, page)) = active {
            let elapsed = previous
                .filter(|(previous_id, _)| *previous_id == id)
                .map_or(Duration::ZERO, |(_, since)| {
                    now.saturating_duration_since(since)
                        .min(SAMPLE + Duration::from_secs(1))
                });
            let ms = elapsed.as_millis() as u64;
            let remainder = elapsed - Duration::from_millis(ms);
            let visit = u64::from(self.last_page != Some((id, page)));
            self.last_page = Some((id, page));
            self.previous = Some((id, now - remainder));
            changed |= ms > 0 || visit > 0;
            if changed {
                for total in [
                    self.by_document.entry(id).or_default(),
                    &mut self.session,
                    &mut *totals,
                ] {
                    total.milliseconds = total.milliseconds.saturating_add(ms);
                    total.page_visits = total.page_visits.saturating_add(visit);
                }
            }
        }
        for total in [&mut self.session, totals] {
            total.focus_sessions = total.focus_sessions.saturating_add(focus_sessions);
        }
        if changed {
            self.save_at.get_or_insert(now + SAVE_INTERVAL);
        }
    }

    pub(super) fn has_pending_save(&self) -> bool {
        self.save_at.is_some()
    }

    pub(super) fn forget(&mut self, id: DocumentId) {
        self.by_document.remove(&id);
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui, history: ReadingTotals) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.scope, Scope::Document, "Documento");
            ui.selectable_value(&mut self.scope, Scope::Session, "Sesión");
            ui.selectable_value(&mut self.scope, Scope::History, "Historial");
        });
        ui.add_space(4.0);
        let totals = match self.scope {
            Scope::Document => {
                let Some((id, page)) = self.current else {
                    ui.label("Abre un documento para consultar su actividad.");
                    return;
                };
                ui.label(
                    egui::RichText::new(format!("Página {} · En esta ventana", page + 1)).small(),
                );
                self.by_document.get(&id).copied().unwrap_or_default()
            }
            Scope::Session => self.session,
            Scope::History => history,
        };
        let seconds = totals.milliseconds / 1000;
        let reading_time = if seconds >= 3600 {
            format!("{} h {:02} min", seconds / 3600, seconds % 3600 / 60)
        } else if seconds >= 60 {
            format!("{} min {:02} s", seconds / 60, seconds % 60)
        } else {
            format!("{seconds} s")
        };
        metric(ui, "Tiempo en primer plano", &reading_time);
        metric(ui, "Visitas a páginas", &totals.page_visits.to_string());
        if self.scope != Scope::Document {
            metric(
                ui,
                "Enfoques completados",
                &totals.focus_sessions.to_string(),
            );
        }
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Las visitas aumentan al abrir o cambiar de página. El historial se guarda en este equipo.")
            .small().color(super::manager::palette(ui).muted));
    }
}

fn metric(ui: &mut egui::Ui, name: &str, value: &str) {
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.label(name);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).monospace().strong());
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_distinct_visits_and_excludes_time_without_a_focused_document() {
        let mut stats = Statistics::default();
        let mut history = ReadingTotals::default();
        let now = Instant::now();
        for (seconds, active, focus) in [
            (0, Some((1, 0)), 0),
            (2, Some((1, 1)), 0),
            (3, None, 1),
            (20, Some((1, 1)), 0),
            (22, Some((1, 1)), 0),
            (23, Some((2, 0)), 0),
        ] {
            stats.record(
                now + Duration::from_secs(seconds),
                active,
                focus,
                &mut history,
            );
        }
        assert_eq!(history.milliseconds, 4_000);
        assert_eq!(history.page_visits, 3);
        assert_eq!(history.focus_sessions, 1);
        assert_eq!(stats.by_document[&1].page_visits, 2);
        assert_eq!(stats.session, history);
    }
}
