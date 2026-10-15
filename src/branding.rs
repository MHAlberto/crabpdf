//! Native application identity. The supplied SVG is rasterized only at build time.

pub const NAME: &str = "CrabPDF";

pub fn viewport() -> eframe::egui::ViewportBuilder {
    eframe::egui::ViewportBuilder::default()
        .with_title(NAME)
        .with_app_id("crabpdf")
        .with_decorations(cfg!(target_os = "macos"))
        .with_fullsize_content_view(cfg!(target_os = "macos"))
        .with_titlebar_shown(!cfg!(target_os = "macos"))
        .with_title_shown(!cfg!(target_os = "macos"))
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/crabpdf.png"
            )))
            .expect("The icon generated during compilation must be a valid PNG"),
        )
}
