use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use simplelog::{ColorChoice, Config, LevelFilter, TermLogger, TerminalMode};

mod commands;
mod modules;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initializes a project
    Init {
        /// Skip GitHub integration
        #[arg(long)]
        skip_github: bool,
        /// Skip Coolify integration
        #[arg(long)]
        skip_coolify: bool,
        /// Skip Trello integration
        #[arg(long)]
        skip_trello: bool,
    },
    /// Checks environment for missing dependencies of the current project
    Check,
    /// Runs predefined scripts. Without arguments shows available scripts
    Run {
        /// Name of the job to run (optional)
        job: Option<String>,
    },
    /// Displays TODOs in code. Similar to `rg TODO`
    Task,
}

fn init_logging(verbose: bool) {
    let level = if verbose {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    let _ = TermLogger::init(
        level,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    );
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    init_logging(cli.verbose);

    match &cli.command {
        Some(Commands::Init { skip_github, skip_coolify, skip_trello }) => {
            commands::init::run(*skip_github, *skip_coolify, *skip_trello)
        }
        Some(Commands::Check) => commands::check::run(),
        Some(Commands::Run { job }) => commands::run::run(job.clone()),
        Some(Commands::Task) => commands::task::run(),
        None => {
            let mut cmd = <Cli as CommandFactory>::command();
            cmd.print_help()
                .map_err(|e| anyhow::anyhow!("Failed to print help: {}", e))
        }
    }
}
