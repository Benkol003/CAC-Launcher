#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{collections::HashMap, env, fs::{self, File, OpenOptions}, io::{stdin, Read, Write}, path::{Path, PathBuf}, process::exit, sync::{atomic::AtomicBool, Arc}, time::Duration};
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
use src_backend::{msgraph::SharedDriveItem, servers, UI::{self, ProgressBarBuffer, TUI}, *};
use tokio::time::sleep;

static CONFIG_URL: &str = "https://github.com/Benkol003/CAC-Config/archive/master.zip";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CACConfig {
    pub username: String,
    pub arma_path: String,
    //shared between all servers that need it. TODO add to servers.json if a server requires a password (Option<bool> with default?)
    pub server_password: String,
    pub enabled_optionals: Vec<String>,
    pub pending_updates: Vec<String>
}


impl CACConfig {
    // fn import_caccore() -> Self {
    //     //TODO: need to find arma directory first / get from user ./CACCore
    // }

    fn default(ui: &mut TUI) -> Result<Self,Error> {
        //find arma
        let mut ap = "./arma3_x64.exe".to_string();
        match std::fs::metadata(&ap) {
            Ok(md) => {
                if(!md.is_file()){
                    return Err(anyhow!("found arma at './arma3_x64.exe' but it is not a file")); //TODO not a fatal error?
                } 
            }
            
            //TODO enter folder instead not path to exe
            Err(_) => {
                loop {
                    ap=ui.popup_text_entry("Enter path to 'arma3_x64.exe'");
                    match std::fs::metadata(&ap) {
                        Ok(md) => {
                            if(!md.is_file()){
                                ui.popup_blocking_prompt(format!("'{}' is not a file, try again.",ap).into());
                                continue;
                            }
                            else{
                                break;
                            }
                        }
                        Err(_) => {
                            ui.popup_blocking_prompt(format!("'{}' does not exist, try again.",ap).into());
                            continue;
                        }   
                    }
                }
            }
        }

        Ok(CACConfig {
            arma_path: ap.into(),
            username: whoami::username(),
            server_password: String::new(),
            enabled_optionals: Vec::new(),
            pending_updates: Vec::new()
        })
    }
}


#[derive(Serialize, Deserialize,Debug,Clone)]
#[serde(untagged)]
pub enum Links {
    single(String),
    multilink(Vec<String>),
}

#[derive(Serialize, Deserialize,Debug,Clone)]
pub struct DLC {
    link: Links,
    pwd: String,
    description: String
}

#[derive(Serialize, Deserialize, Debug, Clone,Default)]
#[serde(rename_all = "camelCase")]
pub struct CACContent {
    mods : HashMap<String,Links>,
    optionals: HashMap<String,Links>,
    //TODO: arma base game
    dlc: HashMap<String,DLC>,
}


//TODO UNFINISHED
async fn update_cac_config(tui: &mut TUI) -> Result<(),Error> {

    let ctx = build_client_ctx()?;
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
    let new_content = serde_json::from_str::<CACContent>(&mut new_conf_content)?;

    // open old existing content.json
    let old_content;
    if file_exists(CONTENT_FILE.as_path())? {
        let mut old_conf_file = File::open(CONTENT_FILE.as_path())?;
        let mut old_conf_content = String::new(); 
        old_conf_file.read_to_string(&mut old_conf_content)?;
        old_content = serde_json::from_str::<CACContent>(&mut old_conf_content)?;
    }else{
        old_content = CACContent::default();
        let f = OpenOptions::new().write(true).create(true).open(CONTENT_FILE.as_path())?;
        serde_json::to_writer_pretty(f, &old_content);
    }


    // find changed stuff
    // TODO well you want to flatten the contents aswell to just name:link pairs
    //TODO might want atomic writes and also always truncate everything over
    let config_file;
    if !file_exists(CONFIG_FILE.as_path())? {
        config_file = CACConfig::default(tui)?;
        let f = OpenOptions::new().write(true).create(true).open(CONFIG_FILE.as_path())?;
        serde_json::to_writer_pretty(f, &config_file);
        
    }
    let mut config_file = File::open(CONFIG_FILE.as_path())?;
    let mut config_content = String::new();
    config_file.read_to_string(&mut config_content);
    let config = serde_json::from_str::<CACConfig>(&config_content)?;

    fs::remove_dir_all(folder_path);

    Ok(())
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");

    //TODO logger doesnt work
    WriteLogger::init(simplelog::LevelFilter::Warn, Config::default(), File::create("CAC-Launcher.log").unwrap());
    let mut tui = TUI::new();

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
        tui.warn_unkown_mod_state();
    }

    let _z7 = FileAutoDeleter::new("7za.exe"); //allows file to be deleted automatically even if theres an error
    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }

    //also tests if any of the config files are broken. If it is, not my problem to fix that
    update_cac_config(tui).await?; //TODO check if mods list was updated and if so, what.
    return Ok(());

    tui.run().await?;
    
    Ok(())
}


