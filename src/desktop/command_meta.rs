use crabpdf_application::commands::Action;

/// Localized labels and platform shortcuts belong to the desktop.
pub trait ActionMetadata {
    fn label(self) -> &'static str;
    fn shortcut(self) -> Option<&'static str>;
}
impl ActionMetadata for Action {
    fn label(self) -> &'static str {
        match self {
            Self::Open => "Abrir documento",
            Self::Home => "Ir a Inicio / biblioteca",
            Self::Close => "Cerrar pestaña",
            Self::CloseOthers => "Cerrar otras pestañas",
            Self::CloseAll => "Cerrar todas las pestañas",
            Self::CloseLeft => "Cerrar pestañas a la izquierda",
            Self::CloseRight => "Cerrar pestañas a la derecha",
            Self::Reopen => "Reabrir último documento cerrado",
            Self::SaveAs => "Guardar una copia",
            Self::Reload => "Recargar documento",
            Self::NewWindow => "Nueva ventana",
            Self::DuplicateWindow => "Duplicar en otra ventana",
            Self::DuplicateTab => "Duplicar en otra pestaña",
            Self::CopyPath => "Copiar ruta del archivo",
            Self::Exit => "Salir",
            Self::Properties => "Propiedades del documento",
            Self::Reveal => "Mostrar en carpeta",
            Self::Previous => "Página anterior",
            Self::Next => "Página siguiente",
            Self::First => "Primera página",
            Self::Last => "Última página",
            Self::GoTo => "Ir a página",
            Self::Back => "Volver",
            Self::Forward => "Avanzar",
            Self::ScrollUp => "Desplazar hacia arriba",
            Self::ScrollDown => "Desplazar hacia abajo",
            Self::ScrollLeft => "Desplazar hacia la izquierda",
            Self::ScrollRight => "Desplazar hacia la derecha",
            Self::ScrollUpPage => "Desplazar una pantalla hacia arriba",
            Self::ScrollDownPage => "Desplazar una pantalla hacia abajo",
            Self::ScrollLeftPage => "Desplazar una pantalla hacia la izquierda",
            Self::ScrollRightPage => "Desplazar una pantalla hacia la derecha",
            Self::ScrollUpHalf => "Desplazar media pantalla hacia arriba",
            Self::ScrollDownHalf => "Desplazar media pantalla hacia abajo",
            Self::ZoomIn => "Aumentar zoom",
            Self::ZoomOut => "Reducir zoom",
            Self::ActualSize => "Tamaño real",
            Self::FitWidth => "Ajustar al ancho",
            Self::FitHeight => "Ajustar al alto",
            Self::FitPage => "Ajustar página",
            Self::CustomZoom => "Zoom personalizado",
            Self::RotateLeft => "Girar a la izquierda",
            Self::RotateRight => "Girar a la derecha",
            Self::Single => "Una página",
            Self::Facing => "Dos páginas",
            Self::Book => "Libro",
            Self::Continuous => "Desplazamiento continuo",
            Self::VerticalReading => "Lectura vertical ajustada al ancho",
            Self::Manga => "Lectura de derecha a izquierda",
            Self::Find => "Buscar texto",
            Self::FindNext => "Siguiente coincidencia",
            Self::FindPrevious => "Coincidencia anterior",
            Self::Copy => "Copiar selección",
            Self::SelectAll => "Seleccionar texto de la página",
            Self::Highlight => "Resaltar selección",
            Self::Underline => "Subrayar selección",
            Self::UndoMarkup => "Deshacer última marca",
            Self::ToggleSidebar => "Mostrar u ocultar panel lateral",
            Self::Thumbnails => "Miniaturas",
            Self::Outline => "Índice del documento",
            Self::Favorites => "Favoritos",
            Self::ToggleFavorite => "Añadir o quitar favorito",
            Self::Theme => "Cambiar tema claro / oscuro",
            Self::Invert => "Invertir colores del documento",
            Self::Grayscale => "Documento en escala de grises",
            Self::Fullscreen => "Pantalla completa",
            Self::Maximize => "Maximizar / restaurar ventana",
            Self::Presentation => "Modo presentación",
            Self::Toolbar => "Mostrar u ocultar barra",
            Self::MenuBar => "Mostrar u ocultar menú",
            Self::Palette => "Paleta de comandos",
            Self::Settings => "Preferencias",
            Self::NextTab => "Pestaña siguiente",
            Self::PreviousTab => "Pestaña anterior",
            Self::MoveTabLeft => "Mover pestaña a la izquierda",
            Self::MoveTabRight => "Mover pestaña a la derecha",
            Self::ClearHistory => "Borrar documentos recientes",
        }
    }
    fn shortcut(self) -> Option<&'static str> {
        Some(match self {
            Self::Open => "Ctrl/⌘ + O",
            Self::Home => "Ctrl/⌘ + Mayús + H",
            Self::Close => "Ctrl/⌘ + W",
            Self::Reopen => "Ctrl/⌘ + Mayús + T",
            Self::SaveAs => "Ctrl/⌘ + S",
            Self::NewWindow => "Ctrl/⌘ + N",
            Self::DuplicateWindow => "Ctrl/⌘ + Mayús + N",
            Self::Find => "Ctrl/⌘ + F",
            Self::FindNext => "F3",
            Self::FindPrevious => "Mayús + F3",
            Self::GoTo => {
                if cfg!(target_os = "macos") {
                    "⌘ + Alt + G"
                } else {
                    "Ctrl + G"
                }
            }
            Self::First => "Inicio",
            Self::Last => "Fin",
            Self::Next => "N",
            Self::Previous => "P",
            Self::ZoomIn => "Ctrl/⌘ + +",
            Self::ZoomOut => "Ctrl/⌘ + −",
            Self::ActualSize => "Ctrl/⌘ + 1",
            Self::FitWidth => "Ctrl/⌘ + 2",
            Self::FitPage => {
                if cfg!(target_os = "macos") {
                    "⌘ + 9"
                } else {
                    "Ctrl + 0"
                }
            }
            Self::CustomZoom => "Ctrl/⌘ + Y",
            Self::Single => "Ctrl/⌘ + 6",
            Self::Facing => "Ctrl/⌘ + 7",
            Self::Book => "Ctrl/⌘ + 8",
            Self::Copy => "Ctrl/⌘ + C",
            Self::SelectAll => "Ctrl/⌘ + A",
            Self::Highlight => "Ctrl/⌘ + Mayús + L",
            Self::Underline => "Ctrl/⌘ + Mayús + U",
            Self::UndoMarkup => "Ctrl/⌘ + Z",
            Self::Palette => "Ctrl/⌘ + K",
            Self::Fullscreen => "F11",
            Self::Presentation => "F5",
            Self::Toolbar => "F8",
            Self::MenuBar => "F9",
            Self::ToggleSidebar => "F12",
            Self::NextTab => "Ctrl + Tab",
            Self::PreviousTab => "Ctrl + Mayús + Tab",
            Self::ScrollDownPage => "Av Pág / Espacio",
            Self::ScrollUpPage => "Re Pág / Mayús + Espacio",
            Self::Back => "Alt + ←",
            Self::Forward => "Alt + →",
            Self::Reload => "R",
            Self::Continuous => "C",
            _ => return None,
        })
    }
}
