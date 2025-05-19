use std::{env, fmt::format, fs::{self, File}, io::{Read, Write}, panic};
use std::process::{Command, Stdio};
use anyhow::{anyhow, Error};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use colored::Colorize;

use src_backend::*;

static Z7_EXE: &[u8] = include_bytes!("7za.exe");

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

//TODO remove duplicates
fn main() -> Result<(),Error> {
    //unpack z7
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

    if urls.len()==0 {
        println!("{}","no URL's provided to download.".yellow());
        return Ok(());
    }

    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }

    for url in urls {
        let item = msgraph::get_shared_drive_item(&ctx.client, &token, &url)?;
        msgraph::download_item(&ctx.client, &token, &item, "./")?;

        let fname = item.name.rsplit_once('.').ok_or(anyhow!("filename is missing extension"))?.0;
        let args = [
            "e",
            "-y",
            //"-o.",
            "-sccUTF-8",
            "-slp",
            "-spf",
            item.name.as_str(),
        ];
        let mut z7_run = Command::new("./7za.exe").args(args).spawn().map_err(|e| anyhow!("failed to start 7zip: {}",e))?;
        //let z7_progress = ProgressBar::new_spinner().with_style(ProgressStyle::with_template("{spinner} {msg:.green.bold}")?);
        //z7_progress.inc(1);
        let error = z7_run.wait()?;
        if !error.success(){
            return Err(anyhow!("failed to extract {}\n (see 7zip log)",item.name));

        }
        //z7_progress.finish();
        println!("{}",format!("Extracted {fname}").green().bold());
        fs::remove_file(&item.name)?;
    }

    fs::remove_file("7za.exe")?;
    
    Ok(())
}
