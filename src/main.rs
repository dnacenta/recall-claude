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
    /// Archive all unarchived JSONL transcripts
    Archive {
        /// Archive all unarchived transcripts found under ~/.claude/projects/
        #[arg(long)]
        all_unarchived: bool,
    },
    /// Search across conversation archives
    Search {
        /// Search query
        query: String,
        /// Number of context lines around each match
        #[arg(short = 'C', long, default_value = "2")]
        context: usize,
        /// Use ranked mode (show files by relevance instead of line matches)
        #[arg(long)]
        ranked: bool,
        /// Max results in ranked mode
        #[arg(long, default_value = "10")]
        max_results: usize,
    },
    /// Save a checkpoint before context compaction (called by PreCompact hook)
    Checkpoint {
        /// Trigger source (e.g. "precompact")
        #[arg(long, default_value = "precompact")]
        trigger: String,
    },
    /// Analyze MEMORY.md and suggest distillation actions
    Distill,
    /// Memory system health dashboard
    Status,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Commands::Init) | None => recall_claude::init::run(),
        Some(Commands::ArchiveSession) => recall_claude::archive::run_from_hook(),
        Some(Commands::Archive { all_unarchived }) => {
            if all_unarchived {
                recall_claude::archive::archive_all_unarchived()
            } else {
                eprintln!("Usage: recall-claude archive --all-unarchived");
                eprintln!("       Archives all unarchived JSONL transcripts.");
                Ok(())
            }
        }
        Some(Commands::Search {
            query,
            context,
            ranked,
            max_results,
        }) => {
            if ranked {
                recall_claude::search::run_ranked(&query, max_results)
            } else {
                recall_claude::search::run(&query, context)
            }
        }
        Some(Commands::Checkpoint { trigger }) => recall_claude::checkpoint::run(&trigger),
        Some(Commands::Distill) => recall_claude::distill::run(),
        Some(Commands::Status) => recall_claude::status::run(),
    };
    if let Err(e) = result {
        eprintln!("\x1b[31merror:\x1b[0m {e}");
        std::process::exit(1);
    }
}
