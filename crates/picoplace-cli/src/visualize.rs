use anyhow::{Context, Result};
use clap::Args;
use picoplace_engine::{placer, svg_generator};
use picoplace_lang::WithDiagnostics;
use picoplace_ui::prelude::*;
use std::path::PathBuf;

use crate::build::collect_files;

#[derive(Args, Debug, Default, Clone)]
#[command(about = "Visualize a Zener design as an SVG layout")]
pub struct VisualizeArgs {
    /// One or more .zen files to visualize.
    /// When omitted, all .zen files in the current directory are processed.
    #[arg(value_name = "PATHS", value_hint = clap::ValueHint::AnyPath)]
    pub paths: Vec<PathBuf>,

    #[arg(long, help = "Skip opening the SVG file after generation")]
    pub no_open: bool,
}

pub fn execute(args: VisualizeArgs) -> Result<()> {
    let zen_paths = collect_files(&args.paths)?;

    if zen_paths.is_empty() {
        let cwd = std::env::current_dir()?;
        anyhow::bail!(
            "No .zen source files found in {}",
            cwd.canonicalize().unwrap_or(cwd).display()
        );
    }

    for zen_path in zen_paths {
        let spinner = Spinner::builder(format!("Visualizing {}", zen_path.display())).start();

        // 1. Evaluate the Zener file to get the Schematic
        let WithDiagnostics {
            output: schematic,
            diagnostics,
        } = picoplace_lang::run(&zen_path);

        let mut has_errors = false;
        if !diagnostics.is_empty() {
            // We need to stop the spinner to print diagnostics cleanly.
            // We can't call .finish() yet, so we suspend it.
            spinner.suspend(|| {
                for diag in diagnostics {
                    if diag.is_error() {
                        has_errors = true;
                    }
                    picoplace_lang::render_diagnostic(&diag);
                }
            });
        }

        if has_errors || schematic.is_none() {
            spinner.error(format!("Failed to build {}", zen_path.display()));
            continue;
        }

        let schematic = schematic.unwrap();

        // 2. Pass the Schematic to the placer
        spinner.set_message("Placing components...");
        let layout = placer::run(&schematic);

        // 3. Generate the SVG
        spinner.set_message("Generating SVG...");
        let output_path = zen_path.with_extension("svg");
        svg_generator::run(&layout, &schematic, &output_path)
            .context("Failed to generate SVG")?;

        spinner.success(format!(
            "Successfully generated visualization: {}",
            output_path.display()
        ));

        // 4. Open the SVG
        if !args.no_open {
            open::that(&output_path).with_context(|| {
                format!("Failed to open SVG file {}", output_path.display())
            })?;
        }
    }

    Ok(())
}