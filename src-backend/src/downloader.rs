use std::fs::OpenOptions;
use std::io::Read;

use anyhow::{anyhow, Error};
use clap::Parser;

use reqwest::Url;
use src_backend::*;
use src_backend::msgraph::FsEntryType;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[clap(flatten)]
    args: ArgsGroup,
}

#[derive(Parser, Debug)]
#[group(required = true, multiple = false)]
struct ArgsGroup {

    #[arg(short, conflicts_with = "url")]
    url_list_file: Option<String>,

    #[arg(index = 1, conflicts_with = "url_list_file")]
    url: Option<String>,
}

fn main() -> Result<(),Error> {
    let args = Args::parse();
    let ctx = build_client_ctx()?;
    let token = msgraph::login(&ctx.client)?;
    let mut urls: Vec<String> = Vec::new();
    if let Some(path) = args.args.url_list_file {
        if !std::fs::exists(&path)? {
            return Err(anyhow!("file {} does not exist",&path));
        }
        let mut file = std::fs::File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        urls.extend(content.lines().map(|x| x.to_string()));

    }else if let Some(url) = args.args.url {
        urls.push(url);
    }

    for url in urls {
        let item = msgraph::get_shared_drive_item(&ctx.client, &token, &url)?;
        msgraph::download_item(&ctx.client, &token, &item, "./tmp")?;
    }
    Ok(())
}
