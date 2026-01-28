#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(unused_variables)]
#![allow(unreachable_code)]


use std::{fs::{self, File, OpenOptions}, io::Write, panic::PanicHookInfo, time::Duration};
use anyhow::{anyhow,Error};
use log::{log, warn};
use ratatui::{style::Stylize, text::Line};
use regex::Regex;
use reqwest::{header::CONTENT_DISPOSITION};
use simplelog::{WriteLogger};
use src_backend::{configs::{Config, *},UI::{self, TUI}, *};
use tokio::time::{sleep, Sleep};
use clap::Parser;

static CONFIG_URL: &str = "https://github.com/Benkol003/CAC-Config/archive/master.zip";

//TODO UNFINISHED
/// downloads the latest config, checks for new or updated mod links, and adds pending updates to the app config.
/// if no config files exist locally then will create them from defaults.
async fn update_cac_config(tui: &mut TUI) -> Result<(),Error> {

    if !TMP_DOWNLOADS_FILE.as_path().is_file() {
        let tmp_manifest = CACDownloadManifest::default();
        tmp_manifest.save()?;
    }
    //TODO clean out tmp folder

    tui.popup_message("fetching latest configuration...");

    let ctx = ClientCtx::build()?;
    let response = ctx.client.get(CONFIG_URL).timeout(TIMEOUT).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!("download URL HTTP error: {}",response.status().as_str()));
    }

    let headers = response.headers().clone();
    let fname = headers.get(CONTENT_DISPOSITION).ok_or(anyhow!("missing CONTENT_DISPOSITION header from file download"))?.to_str()?
    .rsplit_once("filename=").ok_or(anyhow!("rsplit failed"))?.1;
    let fpath = TMP_FOLDER.join(fname);
    {   
        let data = response.bytes().await?;
        let mut file = File::create(&fpath)?;
        file.write_all(&data)?;
    }
    unzip(fpath.to_str().unwrap(),TMP_FOLDER.to_str().unwrap(),None)?;
    fs::remove_file(fpath)?;

    //path to extracted folder
    let regex = Regex::new(r#"^(.*?)\.(?:zip|7z)(?:\.\d{3})?$"#).unwrap();
    let folder_path = TMP_FOLDER.join(regex.captures(fname).unwrap().get(1).unwrap().as_str().to_string());

    let new_content = CACContent::read_from(folder_path.join("content.json"))?;
    let old_content = CACContent::read()?;

    let mut config= match CONFIG_FILE.as_path().is_file() {
        false => {
            let config = CACConfig::default(tui)?;
            config.save()?;
            config
        },
        true => {
            CACConfig::read()?
        }
    };

    // find changed stuff
    // TODO well you want to flatten the contents aswell to just name:link pairs
    //TODO might want atomic writes and also always truncate everything over
    new_content.content_iter().for_each(|nm|{
        let om = old_content.content_map();
        if !om.contains_key(nm.0){
            config.pending_updates.insert(nm.0.clone());
        }else {
            let ol = om.get(nm.0).unwrap();
            if (*ol)!=nm.1{
                config.pending_updates.insert(nm.0.clone());
            }   
            
        }
    });

    config.save()?;

    fs::copy(folder_path.join("content.json"), CONTENT_FILE.as_path())?;
    fs::copy(folder_path.join("servers.json"), SERVERS_FILE.as_path())?;
    fs::remove_dir_all(folder_path)?;

    Ok(())
}

fn panic_handler(info: &PanicHookInfo) {

    let loc = match info.location() {
        None => { "(Unknown)".to_string()},
        Some(s) => {s.to_string()}
    };

    log::error!("panic occured: {:?}",loc);

    let mut tui = TUI::new();
    tui.popup_blocking_prompt(vec![Line::from(vec!["panic occured @: ".light_red(),loc.into(),]),"this location has been added to CAC-Launcher.log".light_yellow().into()].into());
    drop(tui);
    std::process::exit(-1);
}

#[derive(Parser,Debug)]
#[command(version, about)]
struct Args{
    #[arg(long,default_value_t = false, help="don't update the local CAC-Config manifest with a downloaded latest version")]
    no_update: bool
}

const LOG_PATH: &'static str = "CAC-Config/CAC-Launcher.log";

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    WriteLogger::init(simplelog::LevelFilter::Warn, simplelog::Config::default(), File::create(LOG_PATH).unwrap()).unwrap();
    let mut tui = TUI::new();

    std::panic::set_hook(Box::new(panic_handler));

    match fake_main(&mut tui).await {
    Ok(_) => {},
    Err(e) => {
        let bt = e.backtrace();
        log::error!("Error in main: {}", e);
        log::error!("{bt}\n");
        let el = vec![Line::from(vec!["fatal error: ".light_red(),format!("{}",e).into()]),format!("backtrace has been added to {} ",LOG_PATH).light_yellow().into()];
        tui.popup_blocking_prompt(el.into());
        drop(tui);
        std::process::exit(-1);
    }
    };
}

async fn fake_main(tui: &mut TUI) -> Result<(), Error> {

    let args = Args::parse();

    force_create_dir(&CONFIG_FOLDER)?;
    force_create_dir(&CONFIG_FOLDER.join("tmp"))?;

    //unpack 7z to use for the rest of the program.
    let _z7 = FileAutoDeleter::new("7za.exe"); //allows file to be deleted automatically even if theres an error
    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }
    
    if !args.no_update {
        update_cac_config(tui).await?;
    } 

    tui.run().await?;
    tui.exit_logo();

    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}


