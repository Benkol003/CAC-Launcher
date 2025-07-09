#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{collections::{hash_map::Iter, HashMap, HashSet}, env, fs::{self, File, OpenOptions}, io::{stdin, Read, Write}, panic::PanicHookInfo, path::{Path, PathBuf}, process::exit, sync::{atomic::AtomicBool, Arc}, time::Duration};
use anyhow::{anyhow,Error};
use colored::Colorize;
use crossterm::event;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use ratatui::{style::Stylize, text::{Line, ToLine, ToSpan, ToText}};
use regex::Regex;
use reqwest::{header::CONTENT_DISPOSITION, Url};

use serde::{Serialize,Deserialize};
use serde_json::Value;
use simplelog::{Config, WriteLogger};
use src_backend::{configs::{CACConfig, CACContent}, msgraph::SharedDriveItem, servers, UI::{self, ProgressBarBuffer, TUI}, *};
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

static CONFIG_URL: &str = "https://github.com/Benkol003/CAC-Config/archive/master.zip";

//TODO UNFINISHED
/// downloads the latest config, checks for new or updated mod links, and adds pending updates to the app config.
/// if no config files exist locally then will create them from defaults.
async fn update_cac_config(tui: &mut TUI) -> Result<(),Error> {

    tui.popup_message("fetching latest configuration...");

    let ctx = ClientCtx::build()?;
    let response = ctx.client.get(CONFIG_URL).send().await?;

    if(!response.status().is_success()) {
        return Err(anyhow!("download URL HTTP error: {}",response.status().as_str()));
    }

    let headers = response.headers().clone();
    let fname = headers.get(CONTENT_DISPOSITION).ok_or(anyhow!("missing CONTENT_DISPOSITION header from file download"))?.to_str()?
    .rsplit_once("filename=").ok_or(anyhow!("rsplit failed"))?.1;
    let fpath = TMP_FOLDER.join(fname);
    {   
        let data = response.bytes().await?;
        let mut file = File::create(&fpath)?;
        file.write_all(&data);
    }
    unzip(fpath.to_str().unwrap(),TMP_FOLDER.to_str().unwrap(),None)?;
    fs::remove_file(fpath);

    //path to extracted folder
    let regex = Regex::new(r#"^(.*?)\.(?:zip|7z)(?:\.\d{3})?$"#).unwrap();
    let folder_path = TMP_FOLDER.join(regex.captures(fname).unwrap().get(1).unwrap().as_str().to_string());

    let mut new_conf_file = File::open(folder_path.join("content.json"))?;
    let mut new_conf_content = String::new(); 
    new_conf_file.read_to_string(&mut new_conf_content)?;
    let new_content = serde_json::from_str::<CACContent>(&new_conf_content)?;

    // open old existing content.json
    let old_content;
    if CONTENT_FILE.as_path().is_file() {
        let mut old_conf_file = File::open(CONTENT_FILE.as_path())?;
        let mut old_conf_content = String::new(); 
        old_conf_file.read_to_string(&mut old_conf_content)?;
        old_content = serde_json::from_str::<CACContent>(&old_conf_content)?;
    }else{
        old_content = CACContent::default();
        let f = OpenOptions::new().write(true).create(true).open(CONTENT_FILE.as_path())?;
        serde_json::to_writer_pretty(f, &old_content)?;
    }

    let config_file;
    if !CONFIG_FILE.as_path().is_file() {
        config_file = CACConfig::default(tui)?;
        let f = OpenOptions::new().write(true).create(true).open(CONFIG_FILE.as_path())?;
        serde_json::to_writer_pretty(f, &config_file)?;
        
    }
    let mut config_file = File::open(CONFIG_FILE.as_path())?; 
    let mut config_content = String::new();
    config_file.read_to_string(&mut config_content);
    let mut config = serde_json::from_str::<CACConfig>(&config_content)?;


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

    let mut config_file = OpenOptions::new().write(true).truncate(true).open(CONFIG_FILE.as_path())?;
    serde_json::to_writer_pretty(config_file, &config)?;

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
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");

    //TODO logger doesnt work
    WriteLogger::init(simplelog::LevelFilter::Warn, Config::default(), File::create("CAC-Launcher.log").unwrap());
    let mut tui = TUI::new();

    std::panic::set_hook(Box::new(panic_handler));

    match fake_main(&mut tui).await {
    Ok(_) => {},
    Err(e) => {
        let bt = e.backtrace();
        log::error!("Error in main: {}", e);
        log::error!("{bt}\n");
        let el = vec![Line::from(vec!["fatal error: ".light_red(),format!("{}",e).into()]),"backtrace has been added to CAC-Launcher.log".light_yellow().into()];
        tui.popup_blocking_prompt(el.into());
    }
    };
}

async fn fake_main(tui: &mut TUI) -> Result<(), Error> {

    force_create_dir(&CONFIG_FOLDER);
    force_create_dir(&CONFIG_FOLDER.join("tmp"));

    if !std::fs::exists(CONTENT_FILE.as_path())? {
        tui.warn_unknown_mod_state();
    }

    let _z7 = FileAutoDeleter::new("7za.exe"); //allows file to be deleted automatically even if theres an error
    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }

    //also tests if any of the config files are broken. If it is, not my problem to fix that
    update_cac_config(tui).await?; //TODO check if mods list was updated and if so, what.

    tui.run().await?;
    tui.popup_message(UI::LOGO);
    tokio::time::sleep(Duration::from_secs(1)).await;
    Ok(())
}


