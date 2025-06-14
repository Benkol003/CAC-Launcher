#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{env, fs::{self, File}, io::{stdin, Write}, path::{Path, PathBuf}, process::exit, sync::{atomic::AtomicBool, Arc}, time::Duration};
use anyhow::{anyhow,Error};
use colored::Colorize;
use crossterm::event;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::{header::CONTENT_DISPOSITION, Url};

use simplelog::{Config, WriteLogger};
use src_backend::{msgraph::SharedDriveItem, servers, UI::{self, ProgressBarBuffer, TUI}, *};

static CONFIG_URL: &str = "https://github.com/Benkol003/CAC-Config/archive/master.zip";

//TODO handle internet disconnect
async fn update_cac_config() -> Result<String,Error> {

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
    //let folder_name = unzip(fpath.to_str().unwrap(),TMP_FOLDER.to_str().unwrap())?;
    Ok(fname.to_string())
}

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_BACKTRACE", "1");
    //std::env::set_var("RUST_LIB_BACKTRACE", "1");

    //TODO ctrl c doesnt work

    WriteLogger::init(simplelog::LevelFilter::Warn, Config::default(), File::create("CAC-Launcher.log").unwrap());

    match fake_main().await {
    Ok(_) => {},
    Err(e) => {
        let bt = e.backtrace();
        log::error!("{}{}\n{}","Error in main: ".bright_red(), e,"backtrace has been added to 'CAC-Launcher.log'".bright_red());
        let mut f =File::create("CAC-Launcher.log").unwrap();
        log::error!("{e}\n");
        f.write(bt.to_string().as_bytes()); 
        loop {
            let e = crossterm::event::read().unwrap();
            if e.is_key_press() {
                return;
            }
        }
    }
    };
}

async fn fake_main() -> Result<(), Error> {

    let _progressBuf = ProgressBarBuffer::new(50);
    let progressBuf = _progressBuf.buffer.clone();
    let progressTarget = ProgressDrawTarget::term_like(Box::new(_progressBuf));
    let mut z7_progress = ProgressBar::with_draw_target(Some(20),progressTarget)
            .with_style(
            ProgressStyle::with_template("{spinner} Progress: {percent}% Elapsed: {elapsed}, ETA: {eta} {msg:.green.bold} {bar}")?);
    z7_progress.set_message("hello!");
    z7_progress.set_length(100);

    let mut tui = TUI::new();

    //TODO RM
    let  _finish = Arc::new(AtomicBool::new(false));
    let finish = _finish.clone();
    tokio::spawn(async move {
            for i in 0..100 {
            std::thread::sleep(Duration::from_millis(50));
            z7_progress.inc(1);
    }
    _finish.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    tui.popup_progress(progressBuf,finish);
    return Ok(());
    
    force_create_dir(&CONFIG_FOLDER);
    force_create_dir(&CONFIG_FOLDER.join("tmp"));

    if !std::fs::exists(CONFIG_FOLDER.join("config"))? {
        tui.warn_unkown_mod_state();
    }

    let _z7 = FileAutoDeleter::new("7za.exe"); //allows file to be deleted automatically even if theres an error
    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }

    //also tests if any of the config files are broken. If it is, not my problem to fix that
    update_cac_config().await?; //TODO check if mods list was updated and if so, what.

    tui.run().await?;
    
    

    Ok(())
}


