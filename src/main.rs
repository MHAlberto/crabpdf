#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use crabpdf::branding;
use crabpdf::cli::{self, Invocation};
use crabpdf::ui::ReaderApp;

fn main() -> eframe::Result {
    let launch = match cli::parse(std::env::args_os().skip(1)) {
        Ok(Invocation::Run(options)) => options,
        Ok(Invocation::Help) => {
            information(cli::HELP, false);
            return Ok(());
        }
        Ok(Invocation::Version) => {
            information(
                &format!("{} {}", branding::NAME, env!("CARGO_PKG_VERSION")),
                false,
            );
            return Ok(());
        }
        Err(error) => {
            information(&error, true);
            std::process::exit(2);
        }
    };
    let options = eframe::NativeOptions {
        viewport: branding::viewport()
            .with_inner_size([1080.0, 780.0])
            .with_min_inner_size([640.0, 420.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        branding::NAME,
        options,
        Box::new(move |cc| Ok(Box::new(ReaderApp::with_launch(cc, launch)))),
    )
}

fn information(text: &str, error: bool) {
    #[cfg(all(target_os = "windows", not(debug_assertions)))]
    {
        rfd::MessageDialog::new()
            .set_title(branding::NAME)
            .set_description(text)
            .set_level(if error {
                rfd::MessageLevel::Error
            } else {
                rfd::MessageLevel::Info
            })
            .show();
    }
    #[cfg(not(all(target_os = "windows", not(debug_assertions))))]
    {
        if error {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }
}
