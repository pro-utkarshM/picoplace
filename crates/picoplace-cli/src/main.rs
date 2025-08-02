use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::process::Command;

mod build;
mod clean;
mod export; // Renamed from layout
mod fmt;
mod lsp;
mod open;
mod visualize; // New command

#[derive(Parser)]
#[command(name = "picoplace")]
#[command(about = "AI-Accelerated Hardware Design Tools by Cirkitly", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build and validate Zener designs
    #[command(alias = "b")]
    Build(build::BuildArgs),

    /// Export a design to a third-party EDA tool
    #[command(alias = "e")]
    Export(export::ExportArgs),

    /// Generate an SVG visualization of a design's layout
    #[command(alias = "v")]
    Visualize(visualize::VisualizeArgs),

    /// Clean build artifacts
    Clean(clean::CleanArgs),

    /// Format .zen and .star files
    Fmt(fmt::FmtArgs),

    /// Start the Language Server Protocol (LSP) server
    Lsp(lsp::LspArgs),

    /// Open existing layout files in their default application
    #[command(alias = "o")]
    Open(open::OpenArgs),

    /// External subcommands are forwarded to picoplace-<command>
    #[command(external_subcommand)]
    External(Vec<OsString>),
}

fn main() -> anyhow::Result<()> {
    // Initialize logger
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Build(args) => build::execute(args),
        Commands::Export(args) => export::execute(args),
        Commands::Visualize(args) => visualize::execute(args),
        Commands::Clean(args) => clean::execute(args),
        Commands::Fmt(args) => fmt::execute(args),
        Commands::Lsp(args) => lsp::execute(args),
        Commands::Open(args) => open::execute(args),
        Commands::External(args) => {
            if args.is_empty() {
                anyhow::bail!("No external command specified");
            }

            // First argument is the subcommand name
            let command = args[0].to_string_lossy();
            let external_cmd = format!("picoplace-{command}");

            // Try to find and execute the external command
            match Command::new(&external_cmd).args(&args[1..]).status() {
                Ok(status) => {
                    // Forward the exit status
                    if !status.success() {
                        match status.code() {
                            Some(code) => std::process::exit(code),
                            None => anyhow::bail!(
                                "External command '{}' terminated by signal",
                                external_cmd
                            ),
                        }
                    }
                    Ok(())
                }
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        eprintln!("Error: Unknown command '{command}'");
                        eprintln!("No built-in command or external command '{external_cmd}' found");
                        std::process::exit(1);
                    } else {
                        anyhow::bail!(
                            "Failed to execute external command '{}': {}",
                            external_cmd,
                            e
                        )
                    }
                }
            }
        }
    }
}