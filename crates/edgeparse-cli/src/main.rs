//! EdgeParse CLI — PDF-to-structured-data extraction tool.

use std::path::PathBuf;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;
use edgeparse_core::api::config::OutputFormat;
use rayon::prelude::*;

/// EdgeParse: High-performance PDF-to-structured-data extraction
#[derive(Parser, Debug)]
#[command(name = "edgeparse", version, about, long_about = None)]
struct Cli {
    /// Input PDF file path(s)
    #[arg(required = true)]
    input: Vec<PathBuf>,

    /// Output directory
    #[arg(short = 'o', long = "output-dir")]
    output_dir: Option<String>,

    /// Password for encrypted PDFs
    #[arg(short = 'p', long = "password")]
    password: Option<String>,

    /// Output formats (comma-separated: json,text,html,pdf,markdown,markdown-with-html,markdown-with-images)
    #[arg(short = 'f', long = "format")]
    format: Option<String>,

    /// Suppress console logging
    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    /// Disable content safety filters (all,hidden-text,off-page,tiny,hidden-ocg)
    #[arg(long = "content-safety-off")]
    content_safety_off: Option<String>,

    /// Enable PII sanitization
    #[arg(long = "sanitize")]
    sanitize: bool,

    /// Preserve original line breaks
    #[arg(long = "keep-line-breaks")]
    keep_line_breaks: bool,

    /// Replacement character for invalid characters
    #[arg(long = "replace-invalid-chars", default_value = " ")]
    replace_invalid_chars: String,

    /// Use PDF structure tree (tagged PDF)
    #[arg(long = "use-struct-tree")]
    use_struct_tree: bool,

    /// Table detection method (default, cluster)
    #[arg(long = "table-method", default_value = "default")]
    table_method: String,

    /// Reading order algorithm (off, xycut)
    #[arg(long = "reading-order", default_value = "xycut")]
    reading_order: String,

    /// Markdown page separator
    #[arg(long = "markdown-page-separator")]
    markdown_page_separator: Option<String>,

    /// Text page separator
    #[arg(long = "text-page-separator")]
    text_page_separator: Option<String>,

    /// HTML page separator
    #[arg(long = "html-page-separator")]
    html_page_separator: Option<String>,

    /// Image output mode (off, embedded, external)
    #[arg(long = "image-output", default_value = "external")]
    image_output: String,

    /// Image format (png, jpeg)
    #[arg(long = "image-format", default_value = "png")]
    image_format: String,

    /// Image output directory
    #[arg(long = "image-dir")]
    image_dir: Option<String>,

    /// Pages to extract (e.g., "1,3,5-7")
    #[arg(long = "pages")]
    pages: Option<String>,

    /// Include headers/footers in output
    #[arg(long = "include-header-footer")]
    include_header_footer: bool,

    /// Hybrid backend (off, docling-fast)
    #[arg(long = "hybrid", default_value = "off")]
    hybrid: String,

    /// Hybrid triage mode (auto, full)
    #[arg(long = "hybrid-mode", default_value = "auto")]
    hybrid_mode: String,

    /// Hybrid backend URL
    #[arg(long = "hybrid-url")]
    hybrid_url: Option<String>,

    /// Hybrid timeout in milliseconds
    #[arg(long = "hybrid-timeout", default_value = "30000")]
    hybrid_timeout: u64,

    /// Enable fallback on hybrid error
    #[arg(long = "hybrid-fallback")]
    hybrid_fallback: bool,
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    if !cli.quiet {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    // Build processing config
    let config = build_config(&cli);

    // Process each input file in parallel
    let has_errors = AtomicBool::new(false);
    cli.input.par_iter().for_each(|input_path| {
        match edgeparse_core::convert(input_path, &config) {
            Ok(doc) => {
                log::info!(
                    "Processed {} ({} pages)",
                    doc.file_name,
                    doc.number_of_pages
                );
                if let Err(e) = write_outputs(input_path, &doc, &config) {
                    eprintln!("Error writing output for {}: {}", input_path.display(), e);
                    has_errors.store(true, Ordering::Relaxed);
                }
            }
            Err(e) => {
                eprintln!("Error processing {}: {}", input_path.display(), e);
                has_errors.store(true, Ordering::Relaxed);
            }
        }
    });

    if has_errors.load(Ordering::Relaxed) {
        process::exit(1);
    }
}

/// Write converted document in the requested output formats.
fn write_outputs(
    input_path: &std::path::Path,
    doc: &edgeparse_core::models::document::PdfDocument,
    config: &edgeparse_core::api::config::ProcessingConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use edgeparse_core::output;
    use std::io::Write;

    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let out_dir = if let Some(ref dir) = config.output_dir {
        let p = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&p)?;
        p
    } else {
        input_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf()
    };

    for fmt in &config.formats {
        let (ext, content) = match fmt {
            OutputFormat::Json => ("json", output::legacy_json::to_legacy_json_string(doc, stem)?),
            OutputFormat::Text => ("txt", output::text::to_text(doc)?),
            OutputFormat::Html => ("html", output::html::to_html(doc)?),
            OutputFormat::Markdown
            | OutputFormat::MarkdownWithHtml
            | OutputFormat::MarkdownWithImages => ("md", output::markdown::to_markdown(doc)?),
            OutputFormat::Pdf => {
                log::warn!("PDF output not yet implemented, skipping");
                continue;
            }
        };

        let out_path = out_dir.join(format!("{stem}.{ext}"));
        let mut file = std::fs::File::create(&out_path)?;
        file.write_all(content.as_bytes())?;
        log::info!("Wrote {}", out_path.display());
    }

    Ok(())
}

fn build_config(cli: &Cli) -> edgeparse_core::api::config::ProcessingConfig {
    use edgeparse_core::api::config::*;
    use edgeparse_core::api::filter::FilterConfig;

    let formats = if let Some(ref fmt) = cli.format {
        fmt.split(',')
            .filter_map(|s| match s.trim() {
                "json" => Some(OutputFormat::Json),
                "text" => Some(OutputFormat::Text),
                "html" => Some(OutputFormat::Html),
                "pdf" => Some(OutputFormat::Pdf),
                "markdown" => Some(OutputFormat::Markdown),
                "markdown-with-html" => Some(OutputFormat::MarkdownWithHtml),
                "markdown-with-images" => Some(OutputFormat::MarkdownWithImages),
                _ => None,
            })
            .collect()
    } else {
        vec![OutputFormat::Json]
    };

    let mut filter_config = FilterConfig::default();
    if let Some(ref flags) = cli.content_safety_off {
        filter_config.apply_safety_off(flags);
    }

    ProcessingConfig {
        output_dir: cli.output_dir.clone(),
        password: cli.password.clone(),
        formats,
        quiet: cli.quiet,
        sanitize: cli.sanitize,
        keep_line_breaks: cli.keep_line_breaks,
        replace_invalid_chars: cli.replace_invalid_chars.clone(),
        use_struct_tree: cli.use_struct_tree,
        table_method: match cli.table_method.as_str() {
            "cluster" => TableMethod::Cluster,
            _ => TableMethod::Default,
        },
        reading_order: match cli.reading_order.as_str() {
            "off" => ReadingOrder::Off,
            _ => ReadingOrder::XyCut,
        },
        markdown_page_separator: cli.markdown_page_separator.clone(),
        text_page_separator: cli.text_page_separator.clone(),
        html_page_separator: cli.html_page_separator.clone(),
        image_output: match cli.image_output.as_str() {
            "off" => ImageOutput::Off,
            "embedded" => ImageOutput::Embedded,
            _ => ImageOutput::External,
        },
        image_format: match cli.image_format.as_str() {
            "jpeg" => ImageFormat::Jpeg,
            _ => ImageFormat::Png,
        },
        image_dir: cli.image_dir.clone(),
        pages: cli.pages.clone(),
        include_header_footer: cli.include_header_footer,
        hybrid: match cli.hybrid.as_str() {
            "docling-fast" => HybridBackend::DoclingFast,
            _ => HybridBackend::Off,
        },
        hybrid_mode: match cli.hybrid_mode.as_str() {
            "full" => HybridMode::Full,
            _ => HybridMode::Auto,
        },
        hybrid_url: cli.hybrid_url.clone(),
        hybrid_timeout: cli.hybrid_timeout,
        hybrid_fallback: cli.hybrid_fallback,
        filter_config,
    }
}
