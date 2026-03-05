use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "recall-claude",
    about = "Persistent three-layer memory system for Claude Code",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the memory system
    Init,
    /// Archive current session's conversation (called by SessionEnd hook)
    ArchiveSession,
    /// Memory system health dashboard
    Status,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Commands::Init) | None => recall_claude::init::run(),
        Some(Commands::ArchiveSession) => recall_claude::archive::run_from_hook(),
        Some(Commands::Status) => recall_claude::status::run(),
    };
    if let Err(e) = result {
        eprintln!("\x1b[31merror:\x1b[0m {e}");
        std::process::exit(1);
    }
}
