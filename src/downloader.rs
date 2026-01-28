use std::{fs::{self, File}, io::{Read, Write}, path::PathBuf};
use anyhow::{anyhow, Error};
use clap::Parser;
use colored::Colorize;

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Url;
use src_backend::{msgraph::SharedDriveItem, *};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {

    #[arg(short,default_value = "")]
    output_dir: String,
    
    #[clap(flatten)]
    args: ArgsGroup,
}

//TODO: add suppport for also reading the modlist json config
//TODO: add command to clean partial downloads
#[derive(Parser, Debug)]
#[group(required = true, multiple = false)]
struct ArgsGroup {

    #[arg(short, conflicts_with = "url")]
    file_url_list: Option<String>,

    #[arg(index = 1, conflicts_with = "file_url_list")]
    url: Option<Vec<String>>,

}

#[tokio::main]
async fn main() -> Result<(),Error> {
    std::env::set_var("RUST_BACKTRACE", "1");

    let _shutdown = CancellationToken::new();
    let shutdown = _shutdown.clone();
    tokio::spawn(async move {
    tokio::signal::ctrl_c().await.unwrap();
    println!("Ctrl+C recieved, exiting...");
    _shutdown.cancel();
    });


    let args = Args::parse();
    let ctx = ClientCtx::build()?;
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
    urls.iter().map(|u| Url::parse(u).map_err(|e| anyhow!(e))).collect::<Result<Vec<Url>,Error>>()?
    .iter().for_each(|u| {tasks.spawn(msgraph::get_shared_drive_item(ctx.client.clone(), token.clone(),u.clone()));});
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
    // limit number of running downloads. unzip can be parallel, but all previous downloads for a split archive need to be downloaded first
    for item in &items {
        let mut parts: Vec<PathBuf> = Vec::new();
        for part in &item.1 {
            let mut progress = ProgressBar::new(0).with_style(ProgressStyle::with_template(PROGRESS_STYLE_DOWNLOAD)?);//TODO static assert usize::MAX<= u64::MAX
            let p =msgraph::download_item(ctx.client.clone(),token.clone(), part.clone(), args.output_dir.clone(),&mut progress, shutdown.clone()).await?;
            let part = match p {
                Some(p) => p,
                None => {
                    println!("{}",format!("Download cancelled.").bold().bright_yellow());
                    return Ok(());
                }
            };
            parts.push(part);
        }

        //7zip will automatically find and extract the remaining parts
        let mut z7_progress = ProgressBar::new_spinner().with_style(
            ProgressStyle::with_template(PROGRESS_STYLE_EXTRACT)?);
        z7_progress.set_length(100);

        //TODO delete the old folder before unzipping if present
        //TODO double check getting archive .000
        
        unzip(parts.get(0).unwrap().as_os_str().to_str().unwrap(),".",Some(&mut z7_progress))?;
        println!("{}",format!("Extracted {}",&item.1[0].name).bold().green());

        //remove archive or all partial archives
        for p in parts {
            fs::remove_file(p)?;
        }
    }
    
    Ok(())
}