use std::path::PathBuf;

use clap::Parser;
use opentk_search_eval::{
    load_quality_benchmark, run_evaluation, run_quality_benchmark, write_quality_report,
    write_report, SearchEngine,
};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, value_enum)]
    engine: EngineArg,
    #[arg(long, default_value = "fixtures")]
    fixtures: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value = "../../target/search-eval")]
    index_root: PathBuf,
    #[arg(long)]
    deep_quality: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum EngineArg {
    Meilisearch,
    Tantivy,
}

impl From<EngineArg> for SearchEngine {
    fn from(value: EngineArg) -> Self {
        match value {
            EngineArg::Meilisearch => Self::Meilisearch,
            EngineArg::Tantivy => Self::Tantivy,
        }
    }
}

fn main() -> Result<(), opentk_search_eval::SearchEvaluationError> {
    let args = Args::parse();
    let engine = SearchEngine::from(args.engine);
    if args.deep_quality {
        let benchmark = load_quality_benchmark(&args.fixtures)?;
        let report = run_quality_benchmark(
            &benchmark,
            &args
                .index_root
                .join(engine.file_stem())
                .join("deep-quality"),
        )?;
        write_quality_report(&report, &args.out)?;
        return Ok(());
    }
    let report = run_evaluation(
        engine,
        &args.fixtures,
        &args.index_root.join(engine.file_stem()),
    )?;
    write_report(&report, &args.out)?;
    Ok(())
}
