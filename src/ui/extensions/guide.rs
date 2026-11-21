use super::*;

pub(super) struct ReadingGuide {
    visible: bool,
    position: f32,
    height: f32,
}

impl Default for ReadingGuide {
    fn default() -> Self {
        Self {
            visible: false,
            position: 0.38,
            height: 25.0,
        }
    }
}

impl Extensions {
    pub(in crate::ui) fn paint_reading_guide(&self, ui: &egui::Ui, page: Rect) {
        if self.guide.visible
            && !self.selecting
            && self
                .inventory
                .packages
                .iter()
                .any(|p| p.enabled && p.manifest.panel == Some(HostPanel::ReadingGuide))
        {
            self.guide.paint(ui, page);
        }
    }
}

impl ReadingGuide {
    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.visible, "Mostrar franja en la página");
        ui.add_enabled_ui(self.visible, |ui| {
            ui.add(
                egui::Slider::new(&mut self.position, 0.10..=0.90)
                    .custom_formatter(|value, _| format!("{:.0} %", value * 100.0))
                    .text("Posición"),
            );
            ui.add(
                egui::Slider::new(&mut self.height, 12.0..=80.0)
                    .suffix(" pt")
                    .text("Altura"),
            );
        });
        ui.label(
            egui::RichText::new("La franja se mantiene a la misma altura al desplazar el PDF.")
                .small()
                .color(super::manager::palette(ui).muted),
        );
    }

    fn paint(&self, ui: &egui::Ui, page: Rect) {
        let clip = ui.clip_rect();
        let y = clip.top() + clip.height() * self.position;
        let band = Rect::from_min_max(
            egui::pos2(page.left(), y - self.height * 0.5),
            egui::pos2(page.right(), y + self.height * 0.5),
        )
        .intersect(page)
        .intersect(clip);
        if band.is_positive() {
            let accent = super::manager::palette(ui).accent;
            let painter = ui.painter();
            painter.rect_filled(band, 0, accent.linear_multiply(0.14));
            painter.hline(
                band.x_range(),
                band.top(),
                Stroke::new(1.0, accent.linear_multiply(0.65)),
            );
        }
    }
}
