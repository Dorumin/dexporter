#![warn(clippy::all)]

mod args;
mod types;
mod api;
mod fs;
mod update;
mod import;
mod export;
mod download;

use clap::Parser;

use args::DexporterOpts;

#[tokio::main]
async fn main() {
    #[cfg(debug_assertions)]
    {
        eprintln!("Warning: You're running this program in debug mode with assertions on.");
        eprintln!("Some parts will be slow.");
        eprintln!("Consider running a proper release, on release mode, which will parse and serialize files much faster.");
    }

    let options = DexporterOpts::parse();

    match options {
        DexporterOpts::Import(import) => {
            let result = import::do_import(import).await;

            if let Err(e) = result {
                eprintln!("A mistake: {e}");
                eprintln!("Fix it.");
            }
        },
        DexporterOpts::Export(export) => {
            let result = export::do_export(export).await;

            if let Err(e) = result {
                eprintln!("A mistake: {e}");
                eprintln!("Fix it.");
            }
        },
        DexporterOpts::Update(update) => {
            match update::do_update(update).await {
                Ok(()) => {
                    eprintln!("Finished ok?");
                },
                Err(_) => {
                    eprintln!("Something went wrong.");
                }
            }
        },
        DexporterOpts::Download(download) => {
            let result = download::do_download(download).await;

            if let Err(e) = result {
                eprintln!("A mistake: {e}");
                eprintln!("Fix it.");
            }
        }
    }
}
