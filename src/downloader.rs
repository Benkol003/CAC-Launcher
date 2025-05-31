use std::{fs::{self, File}, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}};
use std::process::{Command, Stdio};
use anyhow::{anyhow, Error};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use colored::Colorize;

use src_backend::{msgraph::SharedDriveItem, *};
use tokio::task::JoinSet;

static Z7_EXE: &[u8] = include_bytes!("7za.exe");

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {

    #[arg(short,default_value = "")]
    output_dir: String,
    
    #[clap(flatten)]
    args: ArgsGroup,
}

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
        println!("\t{}",i.0);
        for j in &i.1 {
            println!("\t\t{}",j.name);
        }
    }

    println!("Downloading {} files...",urls.len());

    //TODO 
    // limit number of running downloads. unzip can be parralel, but all previous downloads for a split archive need to be downloaded first
    for item in &items {
        
        for part in &item.1 {
            msgraph::download_item(&ctx.client, &token, &part, &args.output_dir).await?;
        }

        //7zip will automatically find and extract the remaining parts

        //TODO pathbuf is fucking awful.
        unzip([args.output_dir.clone(),item.1[0].name.clone()].iter().collect::<PathBuf>().as_os_str().to_str().ok_or(anyhow!("PathBuf to &str failed"))?)?;
        println!("{}",format!("Extracted {}",&item.1[0].name).bold().green());
        for f in &item.1 {
            fs::remove_file([args.output_dir.clone(),f.name.clone()].iter().collect::<PathBuf>())?;
        }
    }

    fs::remove_file("7za.exe")?;
    
    Ok(())
}


fn unzip(fname: &str) -> Result<(),Error> {
    let args = [
            "e",
            "-y",
            //"-o.",
            "-sccUTF-8",
            "-slp",
            "-spf",
            "-bsp2", //ask 7zip to print progress to stderr
            fname,
        ];
        let mut z7_run = Command::new("./7za.exe").args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().map_err(|e| anyhow!("failed to start 7zip: {}",e))?;

        let z7_progress = ProgressBar::new_spinner().with_style(ProgressStyle::with_template("{spinner} Extracting: {percent}% Elapsed: {elapsed}, ETA: {eta} {msg:.green.bold}")?);
        z7_progress.set_length(100);
        let mut reader = BufReader::new(z7_run.stderr.take().ok_or(anyhow!("failed to get stderr to running process"))?);
        let mut buf = Vec::new();
        while z7_run.try_wait()?.is_none() {
            buf.clear();

            //process 7zip's progress output
            let r = reader.read_until('\r' as u8,&mut buf)?;
            if buf.iter().fold(true, |i,x| { i & (*x!=b'%') }) {
                continue;
            }
            let ln = String::from_utf8(buf.clone())?;
            let ln = ln.rsplit_once('\r').ok_or(anyhow!("failed to split at '\r'"))?.0;

            let (pc,msg) = ln.rsplit_once('%').ok_or(anyhow!("failed to split at %"))?;
            let msg = msg.split_once("-");
            match msg {
                None => {},
                Some(s) => {
                    z7_progress.set_message(s.1.to_string());
                }
            }
            let pc = pc.to_string(); let pci: u64 = pc.trim().parse()?;
            z7_progress.set_position(pci);
            if r==0 {
                break;
            }
        }

        z7_progress.finish_and_clear();

        let error = z7_run.wait()?;
        if !error.success(){
            let mut reader = BufReader::new(z7_run.stdout.ok_or(anyhow!("failed to get stdout to running process"))?);
            let mut z7log = String::new();
            reader.read_to_string(&mut z7log)?;
            return Err(anyhow!("failed to extract {}\n (see 7zip log)",fname));

        }


        // let mut pr = String::new();
        // reader.read_to_string(&mut pr)?;
        // let mut f = std::fs::File::create("7z.log")?;
        // f.write(pr.as_bytes())?;

        Ok(())
}