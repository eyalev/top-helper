use clap::{Parser, Subcommand};
use anyhow::Result;

mod process;
mod window;

#[derive(Parser)]
#[command(name = "top-helper")]
#[command(about = "A CLI tool to monitor system resources and track process contexts")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List processes with resource usage and context information
    List {
        /// Filter by process name
        #[arg(short, long)]
        name: Option<String>,

        /// Show only high memory usage processes (>100MB)
        #[arg(long)]
        high_memory: bool,

        /// Sort by memory usage (desc)
        #[arg(long)]
        sort_memory: bool,

        /// Show top N processes by memory usage
        #[arg(long, conflicts_with = "top_cpu")]
        top_memory: Option<usize>,

        /// Show top N processes by CPU usage
        #[arg(long, conflicts_with = "top_memory")]
        top_cpu: Option<usize>,
    },

    /// Show detailed information about a specific process
    Info {
        /// Process ID or name
        process: String,
    },

    /// Switch to the window containing the specified process
    Switch {
        /// Process ID or name
        process: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::List { name, high_memory, sort_memory, top_memory, top_cpu } => {
            process::list_processes(name.as_deref(), *high_memory, *sort_memory, *top_memory, *top_cpu).await?;
        }
        Commands::Info { process } => {
            process::show_process_info(process).await?;
        }
        Commands::Switch { process } => {
            window::switch_to_process_window(process).await?;
        }
    }

    Ok(())
}