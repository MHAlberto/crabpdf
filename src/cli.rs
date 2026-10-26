use crabpdf_core::document::Zoom;
use crabpdf_core::layout::LayoutMode;
use std::ffi::OsString;
use std::path::PathBuf;

pub const HELP: &str = "CrabPDF — lector local de documentos\n\nUso: crabpdf [opciones] [documento.pdf ...]\n\n--page N              Página inicial (desde 1)\n--zoom P              Porcentaje 8–6400, fit-page, fit-width o fit-height\n--view MODE           single, facing o book\n--continuous          Desplazamiento continuo\n--manga               Lectura de derecha a izquierda\n--rotate DEG          0, 90, 180 o 270 grados\n--search TEXT         Buscar al abrir el documento\n--colors MODE         original, inverted o grayscale\n--position JSON       Ancla de lectura versionada\n--fullscreen          Pantalla completa\n--presentation        Presentación\n--config PATH         Archivo de preferencias TOML\n--help                Mostrar esta ayuda\n--version             Mostrar la versión\n--                    Fin de opciones\n\nTambién se aceptan -page, -zoom, -view, -search, -fullscreen y -presentation.\nLos parámetros todavía no implementados producen un error explícito.";

pub use crabpdf_application::tabs::InitialView;
#[derive(Clone, Debug, Default)]
pub struct LaunchOptions {
    pub files: Vec<PathBuf>,
    pub view: InitialView,
    pub fullscreen: bool,
    pub presentation: bool,
    pub config: Option<PathBuf>,
}
pub enum Invocation {
    Run(LaunchOptions),
    Help,
    Version,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Invocation, String> {
    let mut options = LaunchOptions::default();
    let mut args = args.into_iter();
    let mut positional = false;
    while let Some(arg) = args.next() {
        if positional {
            options.files.push(arg.into());
            continue;
        }
        let Some(text) = arg.to_str() else {
            options.files.push(arg.into());
            continue;
        };
        if text == "--" {
            positional = true;
            continue;
        }
        if !text.starts_with('-') {
            options.files.push(arg.into());
            continue;
        }
        let name = text.trim_start_matches('-');
        let mut value = || -> Result<OsString, String> {
            args.next()
                .ok_or_else(|| format!("Falta el valor de {text}."))
        };
        match name {
            "help" | "h" | "?" => return Ok(Invocation::Help),
            "version" => return Ok(Invocation::Version),
            "page" => {
                let value = value()?;
                let page = value
                    .to_str()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|p| *p > 0)
                    .ok_or_else(|| "La página debe ser un entero desde 1.".to_owned())?;
                options.view.page = Some(page - 1);
            }
            "zoom" => {
                let value = value()?;
                let text = value.to_str().ok_or("Zoom inválido.")?;
                options.view.zoom =
                    Some(match text.to_ascii_lowercase().replace(' ', "-").as_str() {
                        "fit-page" | "fitpage" => Zoom::FitPage,
                        "fit-width" | "fitwidth" => Zoom::FitWidth,
                        "fit-height" | "fitheight" => Zoom::FitHeight,
                        _ => Zoom::Percent(
                            text.trim_end_matches('%')
                                .parse::<u16>()
                                .ok()
                                .filter(|p| (8..=6400).contains(p))
                                .ok_or(
                                    "El zoom debe ser 8–6400, fit-page, fit-width o fit-height.",
                                )?,
                        ),
                    });
            }
            "view" => {
                let value = value()?;
                let text = value
                    .to_str()
                    .ok_or("Modo de lectura inválido.")?
                    .to_ascii_lowercase()
                    .replace([' ', '_'], "-");
                let (mode, continuous) = match text.as_str() {
                    "single" | "single-page" => (LayoutMode::Single, false),
                    "facing" => (LayoutMode::Facing, false),
                    "book" | "book-view" => (LayoutMode::Book, false),
                    "continuous" | "continuous-single-page" => (LayoutMode::Single, true),
                    "continuous-facing" => (LayoutMode::Facing, true),
                    "continuous-book" | "continuous-book-view" => (LayoutMode::Book, true),
                    _ => {
                        return Err(
                            "Modo de lectura: single, facing, book o sus variantes continuous."
                                .into(),
                        );
                    }
                };
                options.view.layout = Some(mode);
                options.view.continuous = Some(continuous);
            }
            "continuous" => options.view.continuous = Some(true),
            "manga" => options.view.manga = Some(true),
            "rotate" => {
                let value = value()?;
                let degrees = value
                    .to_str()
                    .and_then(|v| v.parse::<u16>().ok())
                    .filter(|p| [0, 90, 180, 270].contains(p))
                    .ok_or("La rotación debe ser 0, 90, 180 o 270.")?;
                options.view.rotation = Some((degrees / 90) as u8);
            }
            "search" => {
                options.view.search = Some(
                    value()?
                        .into_string()
                        .map_err(|_| "El texto de búsqueda no es Unicode válido.")?,
                )
            }
            "colors" => {
                options.view.colors = Some(match value()?.to_str() {
                    Some("original") => crabpdf_core::document::PageColors::Original,
                    Some("inverted") => crabpdf_core::document::PageColors::Inverted,
                    Some("grayscale") => crabpdf_core::document::PageColors::Grayscale,
                    _ => return Err("Colores: original, inverted o grayscale.".into()),
                });
            }
            "position" => {
                let json = value()?.into_string().map_err(|_| "Posición inválida.")?;
                let position: crabpdf_core::reading_position::ReadingPosition =
                    serde_json::from_str(&json).map_err(|_| "Posición inválida.")?;
                if position.anchor((1.0, 1.0), usize::MAX).is_none() {
                    return Err("Posición inválida.".into());
                }
                options.view.position = Some(position);
            }
            "config" => options.config = Some(value()?.into()),
            "fullscreen" => options.fullscreen = true,
            "presentation" => options.presentation = true,
            _ => {
                return Err(format!(
                    "Opción desconocida o todavía no implementada: {text}. Usa --help."
                ));
            }
        }
    }
    Ok(Invocation::Run(options))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arguments(text: &[&str]) -> Vec<OsString> {
        text.iter().map(OsString::from).collect()
    }
    #[test]
    fn handles_multiple_unicode_paths_and_reference_aliases() {
        let Invocation::Run(options) = parse(arguments(&[
            "-page",
            "2",
            "-zoom",
            "fit page",
            "-view",
            "continuous facing",
            "--search",
            "lector",
            "C:/Lecturas/niño.pdf",
            "--",
            "-otro.pdf",
        ]))
        .unwrap() else {
            panic!()
        };
        assert_eq!(options.files.len(), 2);
        assert_eq!(options.view.page, Some(1));
        assert_eq!(options.view.zoom, Some(Zoom::FitPage));
        assert_eq!(options.view.layout, Some(LayoutMode::Facing));
        assert_eq!(options.view.continuous, Some(true));
        assert_eq!(options.files[1], PathBuf::from("-otro.pdf"));
    }
    #[test]
    fn rejects_invalid_arguments_instead_of_treating_them_as_files() {
        for args in [
            &["--page", "0"][..],
            &["--zoom", "99999"],
            &["--rotate", "45"],
            &["--page"],
            &["--print-to-default"],
        ] {
            assert!(parse(arguments(args)).is_err());
        }
    }
}
