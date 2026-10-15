use crate::config::Theme;
use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub struct Palette {
    pub canvas: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub muted: Color32,
    pub accent: Color32,
}

pub fn palette(theme: Theme) -> Palette {
    match theme {
        Theme::Light => Palette {
            canvas: Color32::from_rgb(234, 236, 239),
            surface: Color32::from_rgb(250, 250, 251),
            border: Color32::from_rgb(218, 222, 227),
            muted: Color32::from_rgb(109, 117, 130),
            accent: Color32::from_rgb(53, 93, 157),
        },
        Theme::Dark => Palette {
            canvas: Color32::from_rgb(25, 28, 33),
            surface: Color32::from_rgb(34, 38, 44),
            border: Color32::from_rgb(54, 60, 69),
            muted: Color32::from_rgb(160, 168, 181),
            accent: Color32::from_rgb(151, 184, 233),
        },
    }
}

pub fn apply(ctx: &egui::Context, theme: Theme) {
    let p = palette(theme);
    let egui_theme = match theme {
        Theme::Light => egui::Theme::Light,
        Theme::Dark => egui::Theme::Dark,
    };
    let mut visuals = match theme {
        Theme::Light => egui::Visuals::light(),
        Theme::Dark => egui::Visuals::dark(),
    };
    visuals.panel_fill = p.surface;
    visuals.window_fill = p.surface;
    visuals.window_stroke = Stroke::new(1.0, p.border);
    visuals.selection.bg_fill = p.accent.gamma_multiply(0.20);
    visuals.selection.stroke = Stroke::new(1.0, p.accent);
    for widget in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.corner_radius = CornerRadius::same(6);
    }
    ctx.set_theme(egui_theme);
    ctx.set_visuals_of(egui_theme, visuals);
    ctx.style_mut_of(egui_theme, |style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.scroll.fade.strength = 0.0;
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 30.0;
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
    });
}
