//! Small CPU-only benchmark of the real raster-cache fallback lookups.
//! cargo run -p crabpdf-runtime --release --example cache_bench -- report.json
use crabpdf_core::document::{PageColors, RenderKey, Zoom};
use crabpdf_runtime::cache::ByteCache;
use std::{hint::black_box, sync::Arc, time::Instant};

fn key(index: usize) -> RenderKey {
    RenderKey {
        document_id: 1,
        revision: (index / 8) as u64,
        page: index % 8,
        zoom: Zoom::Percent(100),
        viewport_width: 1000,
        viewport_height: 800,
        density_milli: 1000,
        bitmap_side_limit: 4096,
        rotation: 0,
        colors: PageColors::Original,
        region: None,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("Expected an output JSON path")?;
    const ITERATIONS: usize = 40_000;
    let mut rows = Vec::new();
    for entries in [8, 32, 128] {
        let mut cache = ByteCache::new(entries * 4);
        for index in 0..entries {
            cache.insert(key(index), Arc::new(index), 4);
        }
        for operation in ["most_recent", "best_match"] {
            let mut samples = Vec::new();
            // Warm the same data and code before observing it.
            for round in 0..10 {
                let began = Instant::now();
                for index in 0..ITERATIONS {
                    let page = black_box(index % 8);
                    if operation == "most_recent" {
                        black_box(cache.most_recent(|key| key.page == page));
                    } else {
                        black_box(
                            cache.best_match(|key| (key.page == page).then_some(key.revision)),
                        );
                    }
                }
                if round != 0 {
                    samples.push(began.elapsed().as_nanos() as f64 / ITERATIONS as f64);
                }
            }
            samples.sort_by(f64::total_cmp);
            rows.push(serde_json::json!({
                "operation": operation, "entries": entries,
                "iterations_per_sample": ITERATIONS, "n": samples.len(),
                "median_ns": samples[samples.len()/2],
                "min_ns": samples[0], "max_ns": samples[samples.len()-1]
            }));
        }
    }
    let report = serde_json::json!({
        "schema":1, "architecture":std::env::consts::ARCH,
        "scope":"CPU-only cache lookup; same-process warm data. Not PDF opening, UI latency, peak memory or a p95 estimate.",
        "rows":rows
    });
    std::fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
