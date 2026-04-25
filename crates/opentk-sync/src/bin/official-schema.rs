use std::{env, path::PathBuf, process::ExitCode};

use opentk_sync::official_schema::{
    official_xsd_dir, source_gap_report, verify_model_against_dir, OFFICIAL_REPOSITORY,
    PINNED_COMMIT,
};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return ExitCode::FAILURE;
    };

    if command == "check" {
        let source_dir = args
            .next()
            .map_or_else(|| official_xsd_dir().to_path_buf(), PathBuf::from);
        match verify_model_against_dir(&source_dir) {
            Ok(mismatches) if mismatches.is_empty() => {
                println!(
                    "official schema model matches vendored XSDs from {OFFICIAL_REPOSITORY}@{PINNED_COMMIT}"
                );
                let gap_report = source_gap_report();
                if !gap_report.is_empty() {
                    println!("documented source gaps:\n{gap_report}");
                }
                ExitCode::SUCCESS
            }
            Ok(mismatches) => {
                eprintln!("official schema model mismatch:");
                for mismatch in mismatches {
                    eprintln!("- {}: {}", mismatch.category, mismatch.detail);
                }
                ExitCode::FAILURE
            }
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        }
    } else {
        print_usage();
        ExitCode::FAILURE
    }
}

fn print_usage() {
    eprintln!("usage: cargo run -p opentk-sync --bin official-schema -- check [xsd-dir]");
}
