use std::{env, fmt::format, fs::{self, File}, io::{BufRead, BufReader, Read, Write}, panic};
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
        unzip(&item.name)?;
        println!("{}",format!("Extracted {}",&item.name).bold().green());
        fs::remove_file(&item.name)?;
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
            return Err(anyhow!("failed to extract {}\n (see 7zip log)",fname));

        }


        // let mut pr = String::new();
        // reader.read_to_string(&mut pr)?;
        // let mut f = std::fs::File::create("7z.log")?;
        // f.write(pr.as_bytes())?;

        Ok(())
}