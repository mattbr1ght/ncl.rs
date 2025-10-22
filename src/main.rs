use clap::{CommandFactory, Parser, Subcommand};

mod commands;
mod modules;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initializes a project
    Init,
    /// Checks environment for missing dependencies of the current project
    Check,
    /// Runs predefined scrips. Without arguments shows available scripts
    Run,
    /// Displays TODOs in code. Similiar to `rg TODO`
    Task
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let mut ncl_command = <Cli as CommandFactory>::command();

    let _ = match &cli.command {
        Some(Commands::Init) => commands::init::run()?,
        Some(Commands::Check) => commands::check::run()?,
        Some(Commands::Run) => commands::run::run()?,
        Some(Commands::Task) => commands::task::run()?,
        None => ncl_command.print_help().expect("ncl_command should have been created"),
    };

    Ok(())
}
