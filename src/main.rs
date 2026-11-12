//! The craft-bridge command line: `pack` and `inspect`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use craft_bridge::build::{self, BuildOptions};
use craft_bridge::inspect::{self, InspectReport};
use craft_bridge::limits::Limits;
use craft_bridge::pack::{self, PackOptions};

#[derive(Parser)]
#[command(
    name = "craft-bridge",
    version,
    about = "Pack SolidCraft designs into craftx artifacts and inspect them"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Pack a recipe and STEP geometry into a craftx artifact.
    Pack {
        /// SolidCraft script JSON (the recipe).
        #[arg(long)]
        recipe: PathBuf,
        /// STEP fallback geometry.
        #[arg(long)]
        step: PathBuf,
        /// Output artifact path.
        #[arg(long)]
        out: PathBuf,
        /// Exporter omission warnings: a JSON file holding an array of strings.
        /// Without it the manifest records the warnings as not provided.
        #[arg(long)]
        warnings: Option<PathBuf>,
        /// Artifact name (defaults to the output file stem).
        #[arg(long)]
        name: Option<String>,
        /// Replace an existing regular file at the output path.
        #[arg(long)]
        overwrite: bool,
        /// Boundary snapshot as STEP=file: the body state after recipe step N.
        #[arg(long)]
        snapshot: Vec<String>,
        /// The single end-of-recipe snapshot (coarse artifact).
        #[arg(long)]
        final_snapshot: Option<PathBuf>,
    },
    /// Show what an artifact contains.
    Inspect {
        /// The artifact to inspect.
        artifact: PathBuf,
        /// Machine-readable JSON output.
        #[arg(long)]
        json: bool,
    },
    /// Generate a SolidWorks macro and a plan report from an artifact.
    Build {
        /// The artifact to rebuild from.
        artifact: PathBuf,
        /// Output macro path (.swb).
        #[arg(long)]
        out: PathBuf,
        /// Write the machine-readable plan report here.
        #[arg(long)]
        plan: Option<PathBuf>,
        /// Write the human plan report here.
        #[arg(long)]
        plan_text: Option<PathBuf>,
        /// Replace existing regular files at the output paths.
        #[arg(long)]
        overwrite: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let limits = Limits::default();
    let result = match cli.command {
        Command::Pack {
            recipe,
            step,
            out,
            warnings,
            name,
            overwrite,
            snapshot,
            final_snapshot,
        } => {
            let mut snapshots: Vec<(usize, PathBuf)> = Vec::new();
            let mut snapshot_error: Option<String> = None;
            for item in &snapshot {
                match item.split_once('=') {
                    Some((n, path)) => match n.trim().parse::<usize>() {
                        Ok(index) => snapshots.push((index, PathBuf::from(path))),
                        Err(_) => snapshot_error = Some(format!("bad --snapshot index in {item}")),
                    },
                    None => {
                        snapshot_error = Some(format!("--snapshot wants STEP=file, got {item}"))
                    }
                }
            }
            if let Some(message) = snapshot_error {
                eprintln!("error: {message}");
                return ExitCode::FAILURE;
            }
            let snapshot_refs: Vec<(usize, &Path)> =
                snapshots.iter().map(|(i, p)| (*i, p.as_path())).collect();
            let options = PackOptions {
                recipe: &recipe,
                step: &step,
                out: &out,
                warnings: warnings.as_deref(),
                name,
                overwrite,
                snapshots: snapshot_refs,
                final_snapshot: final_snapshot.as_deref(),
            };
            pack::pack(&options, &limits).map(|report| {
                eprintln!(
                    "packed {} ({} commands, {})",
                    report.out.display(),
                    report.command_count,
                    report.manifest.warnings.describe()
                );
            })
        }
        Command::Inspect { artifact, json } => {
            inspect::inspect(&artifact, &limits).and_then(|report: InspectReport| {
                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&report)
                            .map_err(|e| craft_bridge::Error::json("report", e))?
                    );
                } else {
                    print!("{}", inspect::render_human(&report));
                }
                Ok(())
            })
        }
        Command::Build {
            artifact,
            out,
            plan,
            plan_text,
            overwrite,
        } => {
            let options = BuildOptions {
                artifact: &artifact,
                out: &out,
                plan: plan.as_deref(),
                plan_text: plan_text.as_deref(),
                overwrite,
            };
            build::build(&options, &limits).map(|outcome| {
                eprintln!(
                    "built {} ({} bytes; {} mapped, {} degrade-planned, {} fallback-planned, {} not-executed)",
                    out.display(),
                    outcome.macro_bytes,
                    outcome.plan.counts.mapped,
                    outcome.plan.counts.degrade_planned,
                    outcome.plan.counts.fallback_planned,
                    outcome.plan.counts.not_executed
                );
            })
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
