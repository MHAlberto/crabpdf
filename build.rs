use resvg::{tiny_skia, usvg};
use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=crabpdf-logo.svg");
    println!("cargo:rerun-if-changed=build.rs");
    let output = env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR");
    let output = Path::new(&output);
    let svg = fs::read("crabpdf-logo.svg").expect("Cannot read the CrabPDF logo");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default())
        .expect("The CrabPDF logo must be a valid SVG");
    let png = render_icon(&tree, 256);
    fs::write(output.join("crabpdf.png"), &png).expect("Cannot write the native icon");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
        let images: Vec<_> = sizes.iter().map(|&size| render_icon(&tree, size)).collect();
        let mut ico = vec![0, 0, 1, 0];
        ico.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
        let mut offset = (6 + sizes.len() * 16) as u32;
        for (&size, image) in sizes.iter().zip(&images) {
            let dimension = if size == 256 { 0 } else { size as u8 };
            ico.extend_from_slice(&[dimension, dimension, 0, 0]);
            ico.extend_from_slice(&1_u16.to_le_bytes());
            ico.extend_from_slice(&32_u16.to_le_bytes());
            ico.extend_from_slice(&(image.len() as u32).to_le_bytes());
            ico.extend_from_slice(&offset.to_le_bytes());
            offset += image.len() as u32;
        }
        for image in images {
            ico.extend_from_slice(&image);
        }
        let icon_path = output.join("crabpdf.ico");
        fs::write(&icon_path, ico).expect("Cannot write the Windows icon");
        let icon_path = icon_path.to_string_lossy().replace('\\', "/");
        let version = env::var("CARGO_PKG_VERSION").expect("Cargo must provide the version");
        let file_version = format!(
            "{},{},{},0",
            env::var("CARGO_PKG_VERSION_MAJOR").unwrap(),
            env::var("CARGO_PKG_VERSION_MINOR").unwrap(),
            env::var("CARGO_PKG_VERSION_PATCH").unwrap()
        );
        let resource = output.join("crabpdf.rc");
        fs::write(
            &resource,
            format!(
                r#"1 ICON "{icon_path}"
1 VERSIONINFO
FILEVERSION {file_version}
PRODUCTVERSION {file_version}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0
FILEOS 0x40004L
FILETYPE 1
FILESUBTYPE 0
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "FileDescription", "CrabPDF\0"
            VALUE "FileVersion", "{version}\0"
            VALUE "InternalName", "crabpdf\0"
            VALUE "OriginalFilename", "crabpdf.exe\0"
            VALUE "ProductName", "CrabPDF\0"
            VALUE "ProductVersion", "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
            ),
        )
        .expect("Cannot write Windows resources");
        embed_resource::compile_for(resource, ["crabpdf"], embed_resource::NONE)
            .manifest_required()
            .expect("Cannot embed the CrabPDF icon and version in the executable");
    }
}

fn render_icon(tree: &usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("Invalid icon dimensions");
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(
        (size as f32 - tree.size().width() * scale) * 0.5,
        (size as f32 - tree.size().height() * scale) * 0.5,
    );
    resvg::render(tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().expect("Cannot encode the CrabPDF icon")
}
