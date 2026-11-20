//! Deliberately malformed replies for the host's process regression tests.
use crabpdf_extension_api::*;
fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let request: Request = read_message(std::path::Path::new(&args[1]))?;
    match request.command.as_str() {
        "wait" => {
            let scratch = std::env::var_os("CRABPDF_EXTENSION_SCRATCH").unwrap();
            std::fs::write(
                std::path::Path::new(&scratch).join("model.download"),
                b"partial",
            )?;
            std::thread::sleep(std::time::Duration::from_secs(30));
        }
        "wrong_id" => write_message(
            std::path::Path::new(&args[3]),
            &Response {
                api_version: API_VERSION,
                request_id: request.request_id.wrapping_add(1),
                result: Ok(ExtensionOutput {
                    kind: OutputKind::Latex,
                    text: "fixture".into(),
                }),
            },
        )?,
        "oversize" => {
            std::fs::File::create(std::path::Path::new(&args[3]))?
                .set_len(MAX_MESSAGE_BYTES + 1)?;
        }
        _ => {
            return serve(|_| {
                Ok(ExtensionOutput {
                    kind: OutputKind::Latex,
                    text: "x^2".into(),
                })
            });
        }
    }
    Ok(())
}
