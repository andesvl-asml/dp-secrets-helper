use clap::{command, arg, Parser, Subcommand, ValueEnum};
use k8s_openapi::List;
use system_manifests::SystemManifests;

mod system_manifests;

/// Tool to help you manage CDP secrets.
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Local clone of the system manifests repository.
    #[arg(long, short = 's', env = "SYSTEM_MANIFESTS")]
    system_manifests: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Lists all secrets found in rendered environment manifests.
    List {
        // Output format
        #[arg(long, short = 'o', value_enum, default_value = "json")]
        output: ListOutputFormat,
    }
}

#[derive(ValueEnum, Debug, Clone)]
enum ListOutputFormat {
    Json,
    Yaml,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let system_manifests = SystemManifests::new(&cli)?;

    println!("Hello, world!");

    Ok(())
}
