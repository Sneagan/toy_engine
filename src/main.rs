use anyhow::{Context, Result};
use env_logger;
use log::trace;
use std::path::PathBuf;
use std::str::FromStr;
use std::string::ParseError;
use structopt::StructOpt;
mod account;
use account::Account;

/// Optional input data format specifier.
#[derive(Debug, PartialEq, StructOpt)]
enum SourceType {
    CsvFile,
    CsvUrl,
}

impl FromStr for SourceType {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "csv" => SourceType::CsvFile,
            "url" => SourceType::CsvUrl,
            _ => SourceType::CsvFile,
        })
    }
}

/// Data structure used in parsing of command line arguments
#[derive(Debug, StructOpt)]
#[structopt(name = "Toy Engine", about = "Parse CSV path")]
struct Arguments {
    /// Input identifier (CSV file path by default)
    #[structopt(parse(from_os_str))]
    input: std::path::PathBuf,
    /// Output file path (defaults to `stdout` if not present)
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,
    /// Source data type (defaults to CSV file input if not specified)
    #[structopt(short, long)]
    source_type: Option<SourceType>,
}

fn main() -> Result<()> {
    env_logger::init();
    trace!("Parsing command line arguments.");
    let args = Arguments::from_args();
    trace!("Reading data from provided path.");
    let transactions_data = std::fs::read(&args.input)
        .with_context(|| format!("Failed to read file {:?}", &args.input))?;

    // CsvFile is the only supported variant at the moment, but the design can be
    // easily extended.
    if let Some(source_type) = args.source_type {
        match source_type {
            SourceType::CsvFile => {
                Account::accounts_state_from_csv_data(&transactions_data, &mut std::io::stdout())
            }
            _ => Ok(()),
        }
    } else {
        Account::accounts_state_from_csv_data(&transactions_data, &mut std::io::stdout())
    }
}

// Tests

#[cfg(test)]
mod tests {
    use assert_cmd::prelude::*;
    use predicates::prelude::*;
    use std::process::Command;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn file_does_not_exist() -> Result<(), Box<dyn std::error::Error>> {
        init();
        let mut cmd = Command::cargo_bin("toy-engine")?;
        cmd.arg("fail.txt");
        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("No such file or directory"));
        Ok(())
    }
}
