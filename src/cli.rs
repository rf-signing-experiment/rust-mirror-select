use clap::{Parser, Subcommand};
use rust_mirror_select::{Error, Mirror, MirrorContents, MirrorEntry};
use url::Url;

#[derive(Parser)]
#[command(version, about = "Tools for publishing rustup mirror DNS records")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    CreateEntry {
        url: Url,
        #[arg(value_parser = parse_contents)]
        contains: MirrorContents,
    },
}

fn parse_contents(text: &str) -> Result<MirrorContents, String> {
    text.split([',', '|'])
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .try_fold(MirrorContents::empty(), |acc, name| {
            match name.to_ascii_uppercase().as_str() {
                "ALL" => Some(MirrorContents::all()),
                upper => MirrorContents::from_name(upper),
            }
            .map(|flag| acc | flag)
            .ok_or_else(|| {
                format!(
                    "unknown content {name:?}; expected tuf, rustup, toolchain, component or all"
                )
            })
        })
}

fn main() -> Result<(), Error> {
    match Cli::parse().command {
        Command::CreateEntry { url, contains } => {
            println!("{}", MirrorEntry::encode(&Mirror::new(url, contains))?);
        }
    }
    Ok(())
}
