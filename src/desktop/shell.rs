use crabpdf_runtime::paths::config_path;
use std::env;
use std::path::{Path, PathBuf};

pub fn new_window(path: Option<&Path>) -> Result<(), crabpdf_core::document::ReaderError> {
    new_window_with_paths(
        path,
        &crabpdf_runtime::paths::AppPaths::from_environment(config_path()),
    )
}

pub fn new_window_with_paths(
    path: Option<&Path>,
    paths: &crabpdf_runtime::paths::AppPaths,
) -> Result<(), crabpdf_core::document::ReaderError> {
    let executable = env::current_exe().map_err(platform_error)?;
    let mut command = std::process::Command::new(executable);
    if let Some(config) = &paths.config {
        command.arg("--config").arg(config);
    }
    command.env("CRABPDF_DATA_DIR", &paths.data);
    if let Some(path) = path {
        command.arg(path);
    }
    command.spawn().map_err(platform_error)?;
    Ok(())
}

/// Launch a separate process with the exact reading view, without rewriting preferences first.
pub fn document_window_with_paths(
    saved: &crabpdf_application::profile::StoredDocument,
    paths: &crabpdf_runtime::paths::AppPaths,
) -> Result<(), crabpdf_core::document::ReaderError> {
    let mut command =
        document_window_command(env::current_exe().map_err(platform_error)?, saved, paths)?;
    command.spawn().map_err(platform_error)?;
    Ok(())
}

fn document_window_command(
    executable: PathBuf,
    saved: &crabpdf_application::profile::StoredDocument,
    paths: &crabpdf_runtime::paths::AppPaths,
) -> Result<std::process::Command, crabpdf_core::document::ReaderError> {
    use crate::{
        document::{PageColors, Zoom},
        layout::LayoutMode,
    };
    let mut command = std::process::Command::new(executable);
    if let Some(config) = &paths.config {
        command.arg("--config").arg(config);
    }
    command.env("CRABPDF_DATA_DIR", &paths.data);
    let zoom = match saved.zoom {
        Zoom::Percent(percent) => percent.to_string(),
        Zoom::FitWidth => "fit-width".into(),
        Zoom::FitHeight => "fit-height".into(),
        Zoom::FitPage => "fit-page".into(),
    };
    let mode = match (saved.layout, saved.continuous) {
        (LayoutMode::Single, false) => "single",
        (LayoutMode::Facing, false) => "facing",
        (LayoutMode::Book, false) => "book",
        (LayoutMode::Single, true) => "continuous",
        (LayoutMode::Facing, true) => "continuous-facing",
        (LayoutMode::Book, true) => "continuous-book",
    };
    let colors = match saved.colors {
        PageColors::Original => "original",
        PageColors::Inverted => "inverted",
        PageColors::Grayscale => "grayscale",
    };
    command
        .arg("--page")
        .arg(saved.page.saturating_add(1).to_string())
        .arg("--zoom")
        .arg(zoom)
        .arg("--view")
        .arg(mode)
        .arg("--rotate")
        .arg((u16::from(saved.rotation % 4) * 90).to_string())
        .arg("--colors")
        .arg(colors);
    if saved.manga {
        command.arg("--manga");
    }
    if let Some(position) = saved.position {
        command.arg("--position").arg(
            serde_json::to_string(&position)
                .map_err(|error| platform_error(std::io::Error::other(error)))?,
        );
    }
    // A filename beginning with "-" must remain a filename.
    command.arg("--").arg(&saved.path);
    Ok(command)
}

pub fn reveal(path: &Path) -> Result<(), crabpdf_core::document::ReaderError> {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("explorer.exe");
        c.arg(format!("/select,{}", path.display()));
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg("-R").arg(path);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path.parent().unwrap_or(path));
        c
    };
    command.spawn().map_err(platform_error)?;
    Ok(())
}

fn platform_error(error: std::io::Error) -> crabpdf_core::document::ReaderError {
    crabpdf_core::document::ReaderError::new(
        crabpdf_core::document::ErrorKind::File,
        "No se pudo abrir el servicio del sistema.",
    )
    .with_details(error.to_string())
}

#[cfg(test)]
mod window_tests {
    use super::*;
    use crate::{
        application::{profile::StoredDocument, tabs::ReadingTab},
        cli::{self, Invocation},
        document::{DocumentInfo, PageColors, Zoom},
        layout::LayoutMode,
    };
    #[test]
    fn separate_window_round_trips_the_view_and_profile_without_shell_quoting() {
        let paths = crabpdf_runtime::paths::AppPaths::from_environment(Some(
            "profile with spaces/preferences.toml".into(),
        ));
        let position = serde_json::from_value(serde_json::json!({
            "schema":1,"page":5,"page_x":500000,"page_y":750000,"viewport_x":500000,"viewport_y":500000
        })).unwrap();
        for zoom in [
            Zoom::Percent(125),
            Zoom::FitWidth,
            Zoom::FitPage,
            Zoom::FitHeight,
        ] {
            for layout in [LayoutMode::Single, LayoutMode::Facing, LayoutMode::Book] {
                for continuous in [true, false] {
                    let saved = StoredDocument {
                        path: "-libro con espacios y niño.pdf".into(),
                        page: 5,
                        zoom,
                        layout,
                        continuous,
                        rotation: 3,
                        colors: PageColors::Inverted,
                        manga: true,
                        position: Some(position),
                    };
                    let command =
                        document_window_command("crabpdf.exe".into(), &saved, &paths).unwrap();
                    assert_eq!(command.get_program(), "crabpdf.exe");
                    assert!(
                        command
                            .get_envs()
                            .any(|(key, value)| key == "CRABPDF_DATA_DIR"
                                && value == Some(paths.data.as_os_str()))
                    );
                    let Invocation::Run(options) =
                        cli::parse(command.get_args().map(|arg| arg.to_owned())).unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(options.files, vec![saved.path.clone()]);
                    assert_eq!(options.config, paths.config);
                    let mut tab = ReadingTab::new(8, saved.path.clone(), None);
                    tab.startup = Some(options.view);
                    tab.finish_open(
                        8,
                        Ok(DocumentInfo {
                            path: saved.path.clone(),
                            page_count: 10,
                        }),
                        true,
                    );
                    assert_eq!(tab.stored(None), Some(saved));
                }
            }
        }
    }
    #[test]
    fn malformed_or_unknown_positions_and_colors_are_rejected() {
        for value in [
            "{}",
            "not-json",
            r#"{"schema":2,"page":0,"page_x":0,"page_y":0,"viewport_x":0,"viewport_y":0}"#,
            r#"{"schema":1,"page":0,"page_x":1000001,"page_y":0,"viewport_x":0,"viewport_y":0}"#,
        ] {
            assert!(cli::parse(["--position", value].map(Into::into)).is_err());
        }
        assert!(cli::parse(["--colors", "purple"].map(Into::into)).is_err());
    }
}
