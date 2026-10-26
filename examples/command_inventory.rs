//! Export the actual shared action registry, including keyboard hints.
use crabpdf::commands::Action;
use crabpdf::desktop::command_meta::ActionMetadata;
use std::io::Write;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "docs/actions.csv".into());
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "action,label,shortcut,requires_document,status")?;
    for action in Action::ALL {
        let status = match action {
            Action::Copy | Action::SelectAll => "Parcial: selección de una página y permisos PDF",
            Action::Find | Action::FindNext | Action::FindPrevious => {
                "Funcional: búsqueda progresiva, límite 10000 coincidencias"
            }
            _ => "Funcional: conectada al despachador; disponibilidad según contexto",
        };
        let quote = |text: &str| format!("\"{}\"", text.replace('"', "\"\""));
        writeln!(
            file,
            "{:?},{},{},{},{}",
            action,
            quote(action.label()),
            quote(action.shortcut().unwrap_or("")),
            action.needs_document(),
            quote(status)
        )?;
    }
    Ok(())
}
