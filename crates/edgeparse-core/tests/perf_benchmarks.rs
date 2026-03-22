//! Performance benchmark tests — measure key pipeline stage throughput.
//!
//! These are not meant for CI gating, but for manual performance profiling.
//! Run with: `cargo test --test perf_benchmarks -- --nocapture`

use std::time::Instant;

use edgeparse_core::api::config::ProcessingConfig;
use edgeparse_core::models::bbox::BoundingBox;
use edgeparse_core::models::chunks::TextChunk;
use edgeparse_core::models::content::ContentElement;
use edgeparse_core::models::enums::{PdfLayer, TextFormat, TextType};
use edgeparse_core::pipeline::orchestrator::{run_pipeline, PipelineState};

fn make_text_chunk(val: &str, x: f64, y: f64, font_size: f64) -> ContentElement {
    ContentElement::TextChunk(TextChunk {
        value: val.to_string(),
        bbox: BoundingBox::new(Some(0), x, y, x + 80.0, y + font_size),
        font_name: "Helvetica".to_string(),
        font_size,
        font_weight: 400.0,
        italic_angle: 0.0,
        font_color: "#000000".to_string(),
        contrast_ratio: 21.0,
        symbol_ends: vec![],
        text_format: TextFormat::Normal,
        text_type: TextType::Regular,
        pdf_layer: PdfLayer::Main,
        ocg_visible: true,
        index: None,
        page_number: Some(0),
        level: None,
        mcid: None,
    })
}

/// Generate a synthetic page with `n` text chunks simulating body text.
fn generate_page(n: usize) -> Vec<ContentElement> {
    let mut elements = Vec::with_capacity(n);
    let mut y = 800.0;
    let mut x = 72.0;
    for i in 0..n {
        elements.push(make_text_chunk(&format!("Word{}", i), x, y, 12.0));
        x += 82.0;
        if x > 500.0 {
            x = 72.0;
            y -= 14.0;
        }
    }
    elements
}

#[test]
fn bench_pipeline_10_pages_100_elements() {
    let pages: Vec<Vec<ContentElement>> = (0..10).map(|_| generate_page(100)).collect();
    let total_elements: usize = pages.iter().map(|p| p.len()).sum();

    let start = Instant::now();
    let mut state = PipelineState::new(pages, ProcessingConfig::default());
    let _ = run_pipeline(&mut state);
    let elapsed = start.elapsed();

    eprintln!(
        "Pipeline 10 pages x 100 elements ({} total): {:.2}ms",
        total_elements,
        elapsed.as_secs_f64() * 1000.0
    );
    // Should complete in reasonable time
    assert!(
        elapsed.as_secs() < 30,
        "Pipeline took too long: {:?}",
        elapsed
    );
}

#[test]
fn bench_pipeline_50_pages_50_elements() {
    let pages: Vec<Vec<ContentElement>> = (0..50).map(|_| generate_page(50)).collect();
    let total_elements: usize = pages.iter().map(|p| p.len()).sum();

    let start = Instant::now();
    let mut state = PipelineState::new(pages, ProcessingConfig::default());
    let _ = run_pipeline(&mut state);
    let elapsed = start.elapsed();

    eprintln!(
        "Pipeline 50 pages x 50 elements ({} total): {:.2}ms",
        total_elements,
        elapsed.as_secs_f64() * 1000.0
    );
    assert!(
        elapsed.as_secs() < 60,
        "Pipeline took too long: {:?}",
        elapsed
    );
}

#[test]
fn bench_language_detection() {
    use edgeparse_core::utils::language_detector;

    let text = "The quick brown fox jumps over the lazy dog. ".repeat(100);

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = language_detector::detect_language(&text);
    }
    let elapsed = start.elapsed();

    eprintln!(
        "Language detection 1000 iterations: {:.2}ms ({:.2}µs/iter)",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_micros() as f64 / 1000.0
    );
    assert!(elapsed.as_secs() < 10);
}

#[test]
fn bench_text_normalization() {
    use edgeparse_core::utils::text_normalizer;

    let text = "The ﬁrst ﬂoor has the oﬃce and the ﬀteen ﬂags.".repeat(50);

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = text_normalizer::normalize_pdf_text(&text);
    }
    let elapsed = start.elapsed();

    eprintln!(
        "Text normalization 1000 iterations: {:.2}ms ({:.2}µs/iter)",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_micros() as f64 / 1000.0
    );
    assert!(elapsed.as_secs() < 10);
}
