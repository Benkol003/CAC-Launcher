#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{env, fs::{self, File}, io::Write, path::{Path, PathBuf}};
use anyhow::{anyhow,Error};
use reqwest::{header::CONTENT_DISPOSITION, Url};

use src_backend::{msgraph::SharedDriveItem, *, UI::UI,servers};

static CONFIG_URL: &str = "https://github.com/Benkol003/CAC-Config/archive/master.zip";

async fn update_cac_config() -> Result<String,Error> {

    let ctx = build_client_ctx()?;
    let response = ctx.client.get(CONFIG_URL).send().await?;

    if(!response.status().is_success()) {
        return Err(anyhow!("download URL HTTP error: {}",response.status().as_str()));
    }

    let headers = response.headers().clone();
    let fname = headers.get(CONTENT_DISPOSITION).ok_or(anyhow!("missing CONTENT_DISPOSITION header from file download"))?.to_str()?
    .rsplit_once("filename=").ok_or(anyhow!("rsplit failed"))?.1;
    {   
        let data = response.bytes().await?;
        let mut file = File::create(fname)?;
        file.write_all(&data);
    }
    unzip(fname)?;
    fs::remove_file(fname);
    Ok(fname.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Error> {

    if !std::fs::exists(CONFIG_FOLDER)? {
        std::fs::create_dir(CONFIG_FOLDER);
    }else if !std::fs::metadata(CONFIG_FOLDER)?.is_dir(){
        std::fs::remove_file(CONFIG_FOLDER);
        std::fs::create_dir(CONFIG_FOLDER);
    }

    let mut ui = UI::new();
    ui.term.clear();

    if !std::fs::exists(PathBuf::from(CONFIG_FOLDER).join("config"))? {
        ui.warn_unkown_mod_state();
    }

    ui.run().await?;

   

    let mut z7 = FileAutoDeleter::new("7za.exe"); //allows file to be deleted automatically even if theres an error
    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }
    
    update_cac_config().await?; //TODO check if mods list was updated and if so, what.

    Ok(())
}


