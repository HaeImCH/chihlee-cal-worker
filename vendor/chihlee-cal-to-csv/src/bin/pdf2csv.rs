use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

use anyhow::{Context, Result, anyhow};
use chihlee_cal_to_csv::{
    ExtractOptions, ExtractionReport, HeaderMode, PageSelection, QualityMode, TableArea,
    extract_pdf_to_csv,
};
use clap::{Args, Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "pdf2csv",
    version,
    about = "Extract tables from text PDFs into CSV"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Extract tables and write merged CSV output.
    Extract(ExtractArgs),
}

#[derive(Debug, Args)]
struct ExtractArgs {
    /// Input PDF path.
    #[arg(short, long)]
    input: PathBuf,

    /// Output CSV path.
    #[arg(short, long)]
    output: PathBuf,

    /// Page selection like 1-3,5.
    #[arg(long)]
    pages: Option<String>,

    /// Manual table area in format page:x1,y1,x2,y2. Repeatable.
    #[arg(long = "area")]
    areas: Vec<String>,

    /// Output delimiter character.
    #[arg(long, default_value = ",")]
    delimiter: char,

    /// Force header interpretation on first row of each table.
    #[arg(long, conflicts_with = "no_header")]
    has_header: bool,

    /// Disable header interpretation; keep first row as data.
    #[arg(long, conflicts_with = "has_header")]
    no_header: bool,

    /// Minimum cells required per candidate table row.
    #[arg(long, default_value_t = 2)]
    min_cols: usize,

    /// Keep only calendar rows matching M/D or M/D~M/D and emit date,event pairs.
    #[arg(long)]
    clean_calendar: bool,

    /// Drop page column from output CSV.
    #[arg(long = "nopage")]
    no_page: bool,

    /// Drop table_id column from output CSV.
    #[arg(long = "notable")]
    no_table: bool,

    /// Rename col_1,col_2 (example: date,event).
    #[arg(long = "custom-col-name", alias = "custom_col_name")]
    custom_col_name: Option<String>,

    /// Enable verbose warning output.
    #[arg(short, long)]
    verbose: bool,
}

fn parse_custom_col_names(value: &str) -> Result<(String, String)> {
    let (first, second) = value
        .split_once(',')
        .ok_or_else(|| anyhow!("invalid --custom-col-name, expected format: col1,col2"))?;
    let first = first.trim();
    let second = second.trim();
    if first.is_empty() || second.is_empty() {
        anyhow::bail!("invalid --custom-col-name, both names must be non-empty");
    }
    Ok((first.to_string(), second.to_string()))
}

fn parse_options(args: &ExtractArgs) -> Result<ExtractOptions> {
    let pages = args
        .pages
        .as_deref()
        .map(PageSelection::from_str)
        .transpose()
        .map_err(|error| anyhow!("invalid page selection: {error}"))
        .context("failed to parse --pages")?;

    let areas = args
        .areas
        .iter()
        .map(|value| {
            TableArea::from_str(value)
                .map_err(|error| anyhow!("invalid table area: {error}"))
                .with_context(|| format!("failed to parse --area '{value}'"))
        })
        .collect::<Result<Vec<_>>>()?;

    let header_mode = if args.has_header {
        HeaderMode::HasHeader
    } else if args.no_header {
        HeaderMode::NoHeader
    } else {
        HeaderMode::AutoDetect
    };

    if !args.delimiter.is_ascii() {
        anyhow::bail!("delimiter must be a single ASCII character");
    }

    let custom_col_names = args
        .custom_col_name
        .as_deref()
        .map(parse_custom_col_names)
        .transpose()?;

    Ok(ExtractOptions {
        pages,
        areas,
        delimiter: args.delimiter as u8,
        header_mode,
        quality_mode: QualityMode::BestEffort,
        min_cols: args.min_cols,
        clean_calendar: args.clean_calendar,
        no_page: args.no_page,
        no_table: args.no_table,
        custom_col_names,
    })
}

fn log_report(report: &ExtractionReport, verbose: bool) {
    if report.warnings.is_empty() {
        return;
    }

    eprintln!("warning: {} issue(s) detected", report.warnings.len());
    if verbose {
        for warning in &report.warnings {
            eprintln!(
                "  - {:?} page={:?} table_id={:?} confidence={:?}: {}",
                warning.code, warning.page, warning.table_id, warning.confidence, warning.message
            );
        }
    }
}

fn run_extract(args: &ExtractArgs) -> Result<ExtractionReport> {
    let options = parse_options(args)?;
    extract_pdf_to_csv(&args.input, &args.output, &options)
        .with_context(|| format!("failed to extract tables from '{}'", args.input.display()))
}

fn main() -> ExitCode {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("chihlee_cal_to_csv=warn"));
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    let args = std::env::args()
        .map(|arg| match arg.as_str() {
            "-nopage" => "--nopage".to_string(),
            "-notable" => "--notable".to_string(),
            "-custom_col_name" => "--custom-col-name".to_string(),
            _ => arg,
        })
        .collect::<Vec<_>>();
    let cli = Cli::parse_from(args);
    match cli.command {
        Commands::Extract(args) => match run_extract(&args) {
            Ok(report) => {
                log_report(&report, args.verbose);
                if report.row_count > 0 {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(2)
                }
            }
            Err(error) => {
                eprintln!("error: {error:#}");
                ExitCode::from(1)
            }
        },
    }
}
