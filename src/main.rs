use clap::{command, arg, Parser, Subcommand, ValueEnum};

mod system_manifests;

/// Tool to help you manage CDP secrets.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Local clone of the system manifests repository.
    #[arg(long, short = 's')]
    system_manifests: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Lists all secrets found in rendered environment manifests.
    List {
        // Output format
        #[arg(long, short = 'o', value_enum)]
        output: ListOutputFormat,
    }
}

#[derive(ValueEnum, Debug, Clone)]
enum ListOutputFormat {
    Json,
    Yaml,
}

fn main() {
    let cli = Cli::parse();

    println!("Hello, world!");
}
