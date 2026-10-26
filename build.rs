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
        for (name, width, height, sidebar) in [
            ("installer-header.bmp", 150, 57, false),
            ("installer-sidebar.bmp", 164, 314, true),
        ] {
            fs::write(
                output.join(name),
                installer_bitmap(&tree, width, height, sidebar),
            )
            .expect("Cannot write installer branding");
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

fn installer_bitmap(tree: &usvg::Tree, width: u32, height: u32, sidebar: bool) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("Invalid installer dimensions");
    for (index, pixel) in pixmap.pixels_mut().iter_mut().enumerate() {
        let x = (index as u32 % width) as f32 / width as f32;
        let y = (index as u32 / width) as f32 / height as f32;
        let glow = ((1.0 - x) * y * 22.0) as u8;
        *pixel = if sidebar {
            tiny_skia::PremultipliedColorU8::from_rgba(23 + glow, 31 + glow / 2, 43, 255).unwrap()
        } else {
            tiny_skia::PremultipliedColorU8::from_rgba(255, 255, 255, 255).unwrap()
        };
    }
    if sidebar {
        let mut path = tiny_skia::PathBuilder::new();
        for line in 0..7 {
            let y = 222.0 + line as f32 * 10.0;
            path.move_to(30.0, y);
            path.line_to(if line == 6 { 101.0 } else { 132.0 }, y);
        }
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(98, 118, 140, 75);
        pixmap.stroke_path(
            &path.finish().unwrap(),
            &paint,
            &tiny_skia::Stroke {
                width: 2.0,
                ..Default::default()
            },
            tiny_skia::Transform::identity(),
            None,
        );
    }
    let size = if sidebar { 138.0 } else { 55.0 };
    let scale = size / tree.size().width().max(tree.size().height());
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale).post_translate(
            if sidebar { 13.0 } else { 89.0 },
            if sidebar { 42.0 } else { 1.0 },
        ),
        &mut pixmap.as_mut(),
    );
    // Uncompressed 24-bit BMP is supported by the native NSIS UI, no image plugin.
    let stride = (width * 3 + 3) & !3;
    let image_bytes = stride * height;
    let mut bmp = Vec::with_capacity((54 + image_bytes) as usize);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(54 + image_bytes).to_le_bytes());
    bmp.extend_from_slice(&[0; 4]);
    bmp.extend_from_slice(&54_u32.to_le_bytes());
    bmp.extend_from_slice(&40_u32.to_le_bytes());
    bmp.extend_from_slice(&width.to_le_bytes());
    bmp.extend_from_slice(&height.to_le_bytes());
    bmp.extend_from_slice(&1_u16.to_le_bytes());
    bmp.extend_from_slice(&24_u16.to_le_bytes());
    bmp.extend_from_slice(&[0; 24]);
    for y in (0..height).rev() {
        for x in 0..width {
            let pixel = pixmap.pixel(x, y).unwrap();
            bmp.extend_from_slice(&[pixel.blue(), pixel.green(), pixel.red()]);
        }
        bmp.resize(bmp.len() + (stride - width * 3) as usize, 0);
    }
    bmp
}
