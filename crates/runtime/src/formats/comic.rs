use super::{
    Format, archive_entries, convert, extension, extract_entry, import_error, io_error,
    natural_cmp, run,
};
use crate::document::ReaderError;
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};
fn image_entry(path: &Path) -> bool {
    matches!(Format::detect(path), Ok(Format::Image)) && extension(path) != "svg"
}
pub(super) fn normalize_image(source: &Path, target: &Path) -> Result<(), ReaderError> {
    if extension(source) == "dib" {
        use image::ImageDecoder;
        let mut signature = [0; 2];
        let mut file = File::open(source).map_err(io_error)?;
        file.read_exact(&mut signature).map_err(io_error)?;
        if &signature != b"BM" {
            let mut decoder = image::codecs::bmp::BmpDecoder::new_without_file_header(
                std::io::BufReader::new(File::open(source).map_err(io_error)?),
            )
            .map_err(io_error)?;
            let mut limits = image::Limits::default();
            limits.max_alloc = Some(128 * 1024 * 1024);
            limits.max_image_width = Some(32768);
            limits.max_image_height = Some(32768);
            decoder.set_limits(limits).map_err(io_error)?;
            return image::DynamicImage::from_decoder(decoder)
                .map_err(io_error)?
                .save_with_format(target, image::ImageFormat::Png)
                .map_err(io_error);
        }
    }
    let mut reader = image::ImageReader::open(source).map_err(io_error)?;
    reader.set_format(match extension(source).as_str() {
        "ico" => image::ImageFormat::Ico,
        "tga" => image::ImageFormat::Tga,
        _ => image::ImageFormat::Bmp,
    });
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(128 * 1024 * 1024);
    limits.max_image_width = Some(32768);
    limits.max_image_height = Some(32768);
    reader.limits(limits);
    let image = reader.decode().map_err(io_error)?;
    image
        .save_with_format(target, image::ImageFormat::Png)
        .map_err(io_error)
}
pub(super) fn import_comic(source: &Path, output: &Path, work: &Path) -> Result<(), ReaderError> {
    let mut entries: Vec<_> = archive_entries(source, work)?
        .into_iter()
        .filter(|entry| !entry.directory && image_entry(Path::new(&entry.name)))
        .collect();
    // OpenRaster contains layers plus a merged preview, not a collection of comic pages.
    if extension(source) == "ora" {
        entries.retain(|entry| entry.name == "mergedimage.png");
    }
    entries.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    if entries.is_empty() {
        return Err(import_error("El archivo no contiene imágenes compatibles."));
    }
    let pages = work.join("pages");
    fs::create_dir(&pages).map_err(io_error)?;
    let mut names = String::new();
    for (index, entry) in entries.iter().enumerate() {
        let original_extension = extension(Path::new(&entry.name));
        let normalize = matches!(original_extension.as_str(), "ico" | "tga" | "dib");
        let name = format!(
            "pages/{index:05}.{}",
            if normalize {
                "png"
            } else {
                &original_extension
            }
        );
        if normalize {
            let original = work.join(format!("original-{index}.{original_extension}"));
            extract_entry(source, entry, &original, work)?;
            normalize_image(&original, &work.join(&name))?;
            fs::remove_file(original).map_err(io_error)?;
        } else {
            extract_entry(source, entry, &work.join(&name), work)?;
        }
        names.push_str(&name);
        names.push('\n');
    }
    fs::write(work.join("pages.txt"), names).map_err(io_error)?;
    let comic = work.join("pages.cbz");
    run(
        "7z",
        &[
            "a".into(),
            "-tzip".into(),
            "-mx=0".into(),
            "-scsUTF-8".into(),
            comic.as_os_str().into(),
            "@pages.txt".into(),
        ],
        work,
        None,
    )?;
    convert(&[comic], output, work)
}
