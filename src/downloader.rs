use std::{fs::{self, File}, io::{Read, Write}, path::PathBuf};
use anyhow::{anyhow, Error};
use clap::Parser;
use colored::Colorize;

use indicatif::{ProgressBar, ProgressStyle};
use src_backend::{msgraph::SharedDriveItem, *};
use tokio::task::JoinSet;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {

    #[arg(short,default_value = "")]
    output_dir: String,
    
    #[clap(flatten)]
    args: ArgsGroup,
}

//TODO: add suppport for also reading the modlist json config
#[derive(Parser, Debug)]
#[group(required = true, multiple = false)]
struct ArgsGroup {

    #[arg(short, conflicts_with = "url")]
    file_url_list: Option<String>,

    #[arg(index = 1, conflicts_with = "file_url_list")]
    url: Option<Vec<String>>,

}

//TODO remove duplicates
#[tokio::main]
async fn main() -> Result<(),Error> {

    let args = Args::parse();
    let ctx = build_client_ctx()?;
    let token = msgraph::login(&ctx.client).await?;
    let mut urls: Vec<String> = Vec::new();
    if let Some(path) = args.args.file_url_list {
        if !std::fs::exists(&path)? {
            return Err(anyhow!("file {} does not exist",&path));
        }
        let mut file = std::fs::File::open(&path)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        urls.extend(content.lines().map(|x| x.to_string()));

    }else if let Some(mut urls_in) = args.args.url {
        urls.append(&mut urls_in);
    }

    if urls.len()==0 {
        println!("{}","no URL's provided to download.".yellow());
        return Ok(());
    }

    let _z7 = FileAutoDeleter::new("7za.exe"); //allows file to be deleted automatically even if theres an error
    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }

    //grab info first and group partial archives
    println!("Fetching link info...");
    let mut tasks = JoinSet::new(); 
    urls.iter().for_each(|u| {tasks.spawn(msgraph::get_shared_drive_item(ctx.client.clone(), token.clone(), u.clone()));});
    let drive_items: Vec<SharedDriveItem>   = tasks.join_all().await.into_iter().collect::<Result<_,_>>()?;
    let items = group_drive_item_archives(drive_items)?;

    println!("items:");
    for i in &items {
        println!("  {}",i.0);
        for j in &i.1 {
            println!("    \u{22a2}{}",j.name);
        }
    }

    println!("Downloading {} files...",urls.len());

    //TODO 
    // limit number of running downloads. unzip can be parralel, but all previous downloads for a split archive need to be downloaded first
    for item in &items {
        for part in &item.1 {
            let mut progress = ProgressBar::new(0).with_style(ProgressStyle::with_template(PROGRESS_STYLE)?);//TODO static assert usize::MAX<= u64::MAX
            msgraph::download_item(ctx.client.clone(),token.clone(), part.clone(), args.output_dir.clone(),&mut progress).await?;
        }

        //7zip will automatically find and extract the remaining parts

        //TODO pathbuf is fucking awful.
        let mut z7_progress = ProgressBar::new_spinner().with_style(
            ProgressStyle::with_template("{spinner} Extracting: {percent}% Elapsed: {elapsed}, ETA: {eta} {msg:.green.bold}")?);
        z7_progress.set_length(100);
        unzip([args.output_dir.clone(),item.1[0].name.clone()].iter().collect::<PathBuf>().to_str().unwrap(),".",&mut z7_progress)?;
        println!("{}",format!("Extracted {}",&item.1[0].name).bold().green());

        //remove archive or all partial archives
        for f in &item.1 {
            fs::remove_file([args.output_dir.clone(),f.name.clone()].iter().collect::<PathBuf>())?;
        }
    }
    
    Ok(())
}