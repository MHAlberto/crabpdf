//! Reproducible native workload. PDFs remain at their supplied paths.
use crabpdf::{
    app_paths::AppPaths,
    document::{PageColors, RenderKey, Zoom},
    worker::{PdfWorker, WorkerEvent},
};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

fn key(document_id: u64, page: usize) -> RenderKey {
    RenderKey {
        document_id,
        revision: 0,
        page,
        zoom: Zoom::Percent(125),
        viewport_width: 1080,
        viewport_height: 780,
        density_milli: 1000,
        bitmap_side_limit: 2048,
        rotation: 0,
        colors: PageColors::Original,
        region: None,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let output = args.next().ok_or("Usage: workload_probe OUTPUT PDF...")?;
    let books: Vec<_> = args.collect();
    if books.is_empty() || books.len() > 64 {
        return Err("Supply between one and 64 PDF paths".into());
    }
    fs::create_dir_all(&output)?;
    let worker =
        PdfWorker::spawn_for_reader(|| {}, AppPaths::resolve(None, Some(output.join("data"))))?;
    let phase = |tag: &str| fs::write(output.join("phase.txt"), tag);
    phase("opening")?;
    let wait = Duration::from_secs(120);
    let mut documents = Vec::new();
    let mut visits = Vec::new();
    for (index, path) in books.into_iter().enumerate() {
        let id = index as u64 + 1;
        let bytes = path.metadata()?.len();
        let began = Instant::now();
        worker.open(id, path)?;
        let pages = loop {
            match worker.events.recv_timeout(wait)? {
                WorkerEvent::Opened(document, result) if document == id => {
                    break result?.page_count;
                }
                WorkerEvent::Unavailable(error) => return Err(error.into()),
                _ => {}
            }
        };
        let opened_ms = began.elapsed().as_secs_f64() * 1_000.0;
        loop {
            if let WorkerEvent::Details(document, result) = worker.events.recv_timeout(wait)?
                && document == id
            {
                result?;
                break;
            }
        }
        let details_ms = began.elapsed().as_secs_f64() * 1_000.0;
        let first_key = key(id, 0);
        worker.render(first_key)?;
        loop {
            match worker.events.recv_timeout(wait)? {
                WorkerEvent::Rendered(received, result) if received == first_key => {
                    result?;
                    worker.release_render(received)?;
                    break;
                }
                WorkerEvent::Unavailable(error) => return Err(error.into()),
                _ => {}
            }
        }
        let first_raster_ms = began.elapsed().as_secs_f64() * 1_000.0;
        documents.push((id, pages));
        visits.push(serde_json::json!({
            "document": id, "file_bytes": bytes, "pages": pages,
            "opened_ms": opened_ms, "details_ms": details_ms, "first_raster_ms": first_raster_ms,
        }));
    }
    phase("navigation")?;
    let mut rendered = Vec::new();
    for round in 0..2 {
        for &(id, pages) in &documents {
            for page in [0, pages / 2, pages - 1] {
                let render_key = key(id, page);
                let began = Instant::now();
                worker.render(render_key)?;
                loop {
                    match worker.events.recv_timeout(wait)? {
                        WorkerEvent::Rendered(received, result) if received == render_key => {
                            let pixels = result?.rgba.len();
                            worker.release_render(received)?;
                            rendered.push(serde_json::json!({
                                "round": round + 1, "document": id, "page": page + 1,
                                "raster_bytes": pixels, "ready_ms": began.elapsed().as_secs_f64() * 1_000.0
                            }));
                            break;
                        }
                        WorkerEvent::Unavailable(error) => return Err(error.into()),
                        _ => {}
                    }
                }
            }
        }
    }
    let mut latency: Vec<_> = rendered
        .iter()
        .filter_map(|sample| sample["ready_ms"].as_f64())
        .collect();
    latency.sort_by(f64::total_cmp);
    let p95 = latency.get((95 * latency.len()).div_ceil(100).saturating_sub(1));
    phase("all-open")?;
    std::thread::sleep(Duration::from_millis(750));
    for &(id, _) in &documents {
        worker.close(id)?;
    }
    drop(worker);
    phase("closed")?;
    fs::write(
        output.join("timings.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 2, "version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH, "documents": visits, "renders": rendered,
            "navigation_p95_ready_ms": p95,
            "notes": "Native raster readiness, excludes GPU and display presentation.",
        }))?,
    )?;
    std::thread::sleep(Duration::from_millis(750));
    Ok(())
}
