use std::path::PathBuf;

use clap::Parser;
use crate::types::{
    Guild,
    Channel
};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub enum DexporterOpts {
    Import(Import),
    Export(Export),
    Update(Update),
    Download(Download)
}

#[derive(Parser, Debug)]
pub struct Update {
    #[arg(long)]
    pub token: String,

    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>,

    #[arg(long)]
    pub guilds: Vec<String>,

    #[arg(skip)]
    pub state: UpdateState
}

#[derive(Debug, Default)]
pub struct UpdateState {
    pub channels: Vec<Channel>,
    pub guilds: Vec<Guild>
}


#[derive(Parser, Debug)]
pub struct Import {
    #[arg(long)]
    pub file: PathBuf,
}

#[derive(Parser, Debug)]
pub struct Export {
    #[arg(long)]
    pub headers: bool,
}


#[derive(Parser, Debug)]
pub struct Download {
    #[arg(long)]
    pub token: String,

    #[arg(long, value_delimiter = ',')]
    pub channels: Vec<String>
}
