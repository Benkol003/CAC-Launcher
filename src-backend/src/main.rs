#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{env,path::Path};
use anyhow::{anyhow,Error};
use reqwest::Url;

use src_backend::*;

fn main() -> Result<(), Error> {
    //dirhash
    let args: Vec<String> = env::args().collect();
    let p = Path::new(args[1].as_str());
    dirhash::build_dir_manifest(&p,&Path::new("CAC-config/hashes.json"))?;
    return Ok(());

    //let path = Path::new("D:\\SteamLibrary\\steamapps\\common\\Arma 3\\Mods\\@ace");
    //let path = Path::new("D:\\SteamLibrary\\steamapps\\common\\Arma 3\\Mods");
    //let path = Path::new("D:\\SteamLibrary\\steamapps\\common\\Arma 3\\Mods\\@CUPTerrainsMaps"); 
    
    // let mut clock = Stopwatch::start_new();
    // println!("{} hash: {}",path.to_string_lossy(),dirhash::hash_directory(path)?.to_string());
    // clock.stop();
    // println!("hash time: {}ms",clock.elapsed_ms());

    let mut client_ctx = build_client_ctx()?;
  
    let url = "https://tinyurl.com/4hznh2sa";
    let url = "https://0jz1q-my.sharepoint.com/:u:/g/personal/brenner650_0jz1q_onmicrosoft_com/ER8foAfRn8BPp_AqMlDVJNoBlOn17yHic_ixZNbwKWOlng?e=gGBEge";

    let info = get_download_info(&client_ctx,Url::parse(url)?,&Path::new("./tmp/"))?;
    println!("etag: {}",info.etag);
    println!("filename: {}",info.filename);

    // let token = msgraph::login(&clientCtx.client)?;
    // while(true){
    //     msgraph::getSharedDriveItem(&clientCtx.client, token.as_str(), url)?;
    // }
    
    Ok(())
}


