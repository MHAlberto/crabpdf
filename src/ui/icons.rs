use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui};

#[derive(Clone, Copy)]
pub enum Icon {
    Open,
    Previous,
    Next,
    Minus,
    Plus,
    FitWidth,
    Theme,
    Sidebar,
    Document,
}

pub fn button(ui: &mut Ui, icon: Icon, tooltip: &str, enabled: bool, selected: bool) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(30.0, 30.0), Sense::click());
        let visuals = ui.style().interact(&response);
        if response.hovered() || response.has_focus() || selected {
            ui.painter().rect_filled(
                rect,
                6,
                if selected {
                    ui.visuals().selection.bg_fill
                } else {
                    visuals.bg_fill
                },
            );
        }
        paint(ui, icon, rect.shrink(7.0), visuals.fg_stroke.color);
        response
            .on_hover_text(tooltip)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

pub fn paint(ui: &Ui, icon: Icon, rect: Rect, color: Color32) {
    let p = ui.painter();
    let stroke = Stroke::new(1.5, color);
    let point = |x: f32, y: f32| {
        Pos2::new(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
    };
    match icon {
        Icon::Previous | Icon::Next => {
            let (a, b) = if matches!(icon, Icon::Previous) {
                (0.65, 0.35)
            } else {
                (0.35, 0.65)
            };
            line((a, 0.15), (b, 0.5));
            line((b, 0.5), (a, 0.85));
        }
        Icon::Minus | Icon::Plus => {
            line((0.15, 0.5), (0.85, 0.5));
            if matches!(icon, Icon::Plus) {
                line((0.5, 0.15), (0.5, 0.85));
            }
        }
        Icon::Sidebar => {
            p.rect_stroke(rect, 2, stroke, StrokeKind::Inside);
            line((0.35, 0.0), (0.35, 1.0));
        }
        Icon::FitWidth => {
            line((0.0, 0.1), (0.0, 0.9));
            line((1.0, 0.1), (1.0, 0.9));
            line((0.2, 0.5), (0.8, 0.5));
            line((0.2, 0.5), (0.38, 0.3));
            line((0.2, 0.5), (0.38, 0.7));
            line((0.8, 0.5), (0.62, 0.3));
            line((0.8, 0.5), (0.62, 0.7));
        }
        Icon::Theme => {
            p.circle_stroke(rect.center(), rect.width() * 0.27, stroke);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                let direction = egui::vec2(angle.cos(), angle.sin());
                p.line_segment(
                    [
                        rect.center() + direction * rect.width() * 0.39,
                        rect.center() + direction * rect.width() * 0.49,
                    ],
                    stroke,
                );
            }
        }
        Icon::Open => {
            p.add(egui::Shape::line(
                vec![
                    point(0.0, 0.8),
                    point(0.0, 0.2),
                    point(0.4, 0.2),
                    point(0.55, 0.35),
                    point(0.95, 0.35),
                    point(0.95, 0.45),
                ],
                stroke,
            ));
            p.add(egui::Shape::closed_line(
                vec![
                    point(0.0, 0.85),
                    point(0.18, 0.45),
                    point(1.0, 0.45),
                    point(0.8, 0.85),
                ],
                stroke,
            ));
        }
        Icon::Document => {
            p.add(egui::Shape::closed_line(
                vec![
                    point(0.18, 0.05),
                    point(0.62, 0.05),
                    point(0.82, 0.25),
                    point(0.82, 0.95),
                    point(0.18, 0.95),
                ],
                stroke,
            ));
            line((0.62, 0.05), (0.62, 0.25));
            line((0.62, 0.25), (0.82, 0.25));
            line((0.32, 0.5), (0.68, 0.5));
            line((0.32, 0.65), (0.68, 0.65));
        }
    }
}
