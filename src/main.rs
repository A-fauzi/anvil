mod build;
mod clean;
mod dev;
mod doctor;
mod environment;
mod generator;
mod install;
mod logs;
mod project;
mod ui;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "anvil", version, about = "Forge your Android applications")]
struct Cli {
    /// Show detailed output from internal build tools
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check Android development environment
    Doctor,

    /// Generate a new Android project
    New {
        /// Project name
        name: String,
    },

    /// Build, install, launch and stream application logs
    Dev {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },

    /// Build Android application
    Build {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },

    /// Install application on connected device
    Install {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },

    /// Show logs for the running Android application
    Logs {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },

    /// Clean build artifacts
    Clean {
        #[arg(long, default_value = ".")]
        project_dir: PathBuf,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let verbose = cli.verbose;

    match cli.command {
        Commands::Doctor => doctor::run(),
        Commands::New { name } => generator::run(&name),
        Commands::Dev { project_dir } => dev::run(&project_dir, verbose),
        Commands::Build { project_dir } => build::run(&project_dir, verbose),
        Commands::Install { project_dir } => install::run(&project_dir, verbose),
        Commands::Logs { project_dir } => logs::run(&project_dir),
        Commands::Clean { project_dir } => clean::run(&project_dir),
    }
}

fn main() {
    if let Err(error) = run() {
        ui::fatal(&error);
        std::process::exit(1);
    }
}
