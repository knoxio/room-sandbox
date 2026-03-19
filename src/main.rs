mod commands;
mod config;
mod docker;
mod state;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "room-sandbox", about = "Dockerized multi-agent sandbox for room")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a new sandbox environment
    Init(commands::init::InitArgs),

    /// Apply changes from sandbox.toml
    Apply,

    /// Manage agents
    #[command(subcommand)]
    Agent(AgentCommand),

    /// Open the room TUI
    Tui {
        /// Username to join as (defaults to system user)
        #[arg(long = "as")]
        username: Option<String>,
    },

    /// Open a shell in the container
    Shell {
        /// Agent name (opens their workspace directory)
        name: Option<String>,

        /// Log in as root instead of agent user
        #[arg(long)]
        root: bool,
    },

    /// Start the sandbox container
    Up,

    /// Stop the sandbox container
    Down,

    /// Tail sandbox container logs
    Logs,

    /// Run Claude Code interactively in an agent's workspace
    Claude {
        /// Agent name
        name: String,

        /// Extra arguments passed to claude (after --)
        #[arg(last = true)]
        claude_args: Vec<String>,
    },

    /// Remove all generated artifacts, keep sandbox.toml
    Clean,
}

#[derive(Subcommand)]
enum AgentCommand {
    /// Add an agent to the sandbox
    Add {
        /// Agent name
        name: String,
    },

    /// Remove an agent from the sandbox
    Remove {
        /// Agent name
        name: String,
    },

    /// List all agents and their status
    List,

    /// Start agent ralph loops
    Start {
        /// Agent name(s)
        names: Vec<String>,

        /// Start all agents
        #[arg(short, long)]
        all: bool,

        /// Tail agent output (default: start in background)
        #[arg(short, long)]
        tail: bool,

        /// Extra arguments passed to room-ralph (after --)
        #[arg(last = true)]
        ralph_args: Vec<String>,
    },

    /// Stop agent ralph loops
    Stop {
        /// Agent name(s)
        names: Vec<String>,

        /// Stop all agents
        #[arg(short, long)]
        all: bool,
    },

    /// Restart agent ralph loops
    Restart {
        /// Agent name(s)
        names: Vec<String>,

        /// Restart all agents
        #[arg(short, long)]
        all: bool,

        /// Tail agent output (default: start in background)
        #[arg(short, long)]
        tail: bool,

        /// Extra arguments passed to room-ralph (after --)
        #[arg(last = true)]
        ralph_args: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Init(args) => commands::init::run(args),
        Command::Apply => commands::apply::run(),
        Command::Agent(cmd) => match cmd {
            AgentCommand::Add { name } => commands::agent::add(&name),
            AgentCommand::Remove { name } => commands::agent::remove(&name),
            AgentCommand::List => commands::agent::list(),
            AgentCommand::Start { names, all, tail, ralph_args } => {
                commands::agent::start(&names, all, tail, &ralph_args)
            }
            AgentCommand::Stop { names, all } => commands::agent::stop(&names, all),
            AgentCommand::Restart { names, all, tail, ralph_args } => {
                commands::agent::restart(&names, all, tail, &ralph_args)
            }
        },
        Command::Tui { username } => commands::tui::run(username.as_deref()),
        Command::Claude { name, claude_args } => commands::claude::run(&name, &claude_args),
        Command::Shell { name, root } => commands::shell::run(name.as_deref(), root),
        Command::Up => commands::up::run(),
        Command::Down => commands::down::run(),
        Command::Logs => commands::logs::run(),
        Command::Clean => commands::clean::run(),
    }
}
