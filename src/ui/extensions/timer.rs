use super::*;

use crabpdf_extension_api::TimerSpec;
use std::time::{Duration, Instant};

pub(super) struct Timer {
    pub(super) settings: TimerSpec,
    remaining: Duration,
    deadline: Option<Instant>,
    resting: bool,
    completed: u64,
}

pub(super) struct Completion {
    pub(super) focus: bool,
    pub(super) message: &'static str,
}

impl Timer {
    pub(super) fn new(settings: TimerSpec) -> Self {
        Self {
            settings,
            remaining: Duration::from_secs(u64::from(settings.focus_minutes) * 60),
            deadline: None,
            resting: false,
            completed: 0,
        }
    }

    fn duration(&self) -> Duration {
        Duration::from_secs(
            u64::from(if self.resting {
                self.settings.break_minutes
            } else {
                self.settings.focus_minutes
            }) * 60,
        )
    }

    pub(super) fn tick(&mut self, ctx: &egui::Context) -> Option<Completion> {
        let completion = self.advance(Instant::now());
        if completion.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }
        if self.deadline.is_some() {
            ctx.request_repaint_after(self.remaining.min(Duration::from_secs(1)));
        }
        completion
    }

    fn advance(&mut self, now: Instant) -> Option<Completion> {
        self.remaining = self.deadline?.saturating_duration_since(now);
        if !self.remaining.is_zero() {
            return None;
        }
        self.deadline = None;
        let focus = !self.resting;
        self.resting = focus;
        if focus {
            self.completed = self.completed.saturating_add(1);
        }
        self.remaining = self.duration();
        Some(Completion {
            focus,
            message: if focus {
                "Pomodoro completado. El descanso está listo."
            } else {
                "Descanso completado. Puedes iniciar otro enfoque."
            },
        })
    }

    pub(super) fn clock(&self) -> String {
        let secs = self.remaining.as_secs() + u64::from(self.remaining.subsec_nanos() != 0);
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    pub(super) fn phase(&self) -> &'static str {
        if self.resting { "Descanso" } else { "Enfoque" }
    }

    pub(super) fn running(&self) -> bool {
        self.deadline.is_some()
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        let colors = super::manager::palette(ui);
        let progress =
            (1.0 - self.remaining.as_secs_f32() / self.duration().as_secs_f32()).clamp(0.0, 1.0);
        ui.vertical_centered(|ui| {
            ui.add_space(4.0);
            let diameter = ui.available_width().clamp(106.0, 142.0);
            let (rect, dial) =
                ui.allocate_exact_size(egui::Vec2::splat(diameter), egui::Sense::hover());
            let radius = diameter * 0.45;
            ui.painter()
                .circle_stroke(rect.center(), radius, Stroke::new(4.0, colors.border));
            if progress > 0.0 {
                let points: Vec<_> = (0..=48)
                    .map(|step| {
                        let angle = progress * std::f32::consts::TAU * step as f32 / 48.0
                            - std::f32::consts::FRAC_PI_2;
                        rect.center() + egui::vec2(angle.cos(), angle.sin()) * radius
                    })
                    .collect();
                ui.painter()
                    .add(egui::Shape::line(points, Stroke::new(4.0, colors.accent)));
            }
            let clock = self.clock();
            dial.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Label,
                    true,
                    format!("{} {clock}", self.phase()),
                )
            });
            ui.painter().text(
                rect.center() - egui::vec2(0.0, 10.0),
                egui::Align2::CENTER_CENTER,
                clock,
                egui::FontId::monospace(31.0),
                ui.visuals().text_color(),
            );
            ui.painter().text(
                rect.center() + egui::vec2(0.0, 19.0),
                egui::Align2::CENTER_CENTER,
                self.phase(),
                egui::FontId::proportional(12.0),
                colors.muted,
            );
            ui.add_space(6.0);
            let action = if self.deadline.is_some() {
                "Pausar"
            } else if self.remaining < self.duration() {
                "Continuar"
            } else if self.resting {
                "Iniciar descanso"
            } else {
                "Iniciar enfoque"
            };
            if super::manager::primary(ui, true, action).clicked() {
                let now = Instant::now();
                if let Some(deadline) = self.deadline.take() {
                    self.remaining = deadline.saturating_duration_since(now);
                } else {
                    self.deadline = Some(now + self.remaining);
                }
                ui.ctx().request_repaint();
            }
            if ui
                .add_enabled(
                    progress > 0.0 || self.running(),
                    egui::Button::new("Reiniciar fase").frame(false),
                )
                .clicked()
            {
                self.remaining = self.duration();
                self.deadline = None;
            }
        });
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!("{} min de enfoque", self.settings.focus_minutes))
                    .small(),
            );
            ui.label(
                egui::RichText::new(format!("{} min de descanso", self.settings.break_minutes))
                    .small(),
            );
        });
        ui.label(
            egui::RichText::new(format!(
                "Enfoques completados en esta sesión: {}",
                self.completed
            ))
            .small()
            .color(colors.muted),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_focus_moves_to_a_stopped_break_and_counts_once() {
        let mut timer = Timer::new(TimerSpec {
            focus_minutes: 25,
            break_minutes: 5,
        });
        let now = Instant::now();
        assert_eq!(timer.clock(), "25:00");
        timer.deadline = Some(now);
        assert!(timer.advance(now).unwrap().focus);
        assert_eq!(timer.clock(), "05:00");
        assert_eq!(timer.completed, 1);
        assert!(!timer.running());
        assert!(timer.advance(now).is_none());
        timer.deadline = Some(now);
        assert!(!timer.advance(now).unwrap().focus);
        assert_eq!(timer.clock(), "25:00");
        assert_eq!(timer.completed, 1);
    }
}
