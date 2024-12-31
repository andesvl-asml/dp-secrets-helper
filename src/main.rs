use clap::{arg, command, Parser, Subcommand, ValueEnum};
use system_manifests::{FlatManifestResource, SystemManifests};

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
    },
}

#[derive(ValueEnum, Debug, Clone)]
enum ListOutputFormat {
    Json,
    Yaml,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let system_manifests = SystemManifests::new(&cli)?;

    match cli.command {
        Commands::List { output } => {
            let mut secret_resource_manifests = Vec::new();
            for manifest_resource_result in system_manifests.resource_iter() {
                let manifest_resource = manifest_resource_result?;
                if let Some(t) = &manifest_resource.resource.types {
                    match t.kind.as_str() {
                        "Secret" | "ExternalSecret" | "PushSecret" => {
                            secret_resource_manifests.push(manifest_resource)
                        }
                        _ => (),
                    }
                }
            }

            let secret_resource_manifest_flat: Vec<FlatManifestResource> = secret_resource_manifests
                .into_iter()
                .map(|srm| srm.into())
                .collect();

            let stdout = std::io::stdout();
            let mut writer = std::io::BufWriter::new(stdout.lock());

            match output {
                ListOutputFormat::Json => {
                    serde_json::to_writer(&mut writer, &secret_resource_manifest_flat)?
                }
                ListOutputFormat::Yaml => {
                    serde_yaml::to_writer(&mut writer, &secret_resource_manifest_flat)?
                }
            };
        }
    };
    Ok(())
}
