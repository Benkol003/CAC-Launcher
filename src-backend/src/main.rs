#![allow(warnings)] //for debugging only, RM

use std::{sync::Mutex,default, env, fmt::Debug, io::Write, path::Path, str::FromStr, sync::Arc, time::SystemTime, usize};
use anyhow::{anyhow,Error};
use base64::display;
use reqwest::{blocking::{Client, Request, Response}, header, redirect::Policy,Url};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex, CookieStoreRwLock};
use stopwatch::Stopwatch;
use tokio_util::sync::CancellationToken;

mod secrets;


use jwalk::WalkDir;

//app will be blocked without this. reccommend using a browser user agent string to prevent rate limiting.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";

struct DownloadInfo {
        sessionUrl: Url,
        downloadUrl: Url,
        filename: String,
        fileSize: usize,
        etag: String,
}

//download until finished or canceelled. If file exists at path then resumes a partial download.
async fn download_file(info: DownloadInfo,path: &Path,canceller: CancellationToken) -> Result<(),Error> {
    let do_partial = Path::exists(path);

    return Ok(());
}

//using reqwest
fn get_download_info(ctx: &ClientCtx,url: Url,dir: &Path) -> Result<DownloadInfo,Error> {

    //make sure to clear out FedAuth cookie so it doesnt break stuff
    {
        let mut jarlock = ctx.jar.write().unwrap();
        jarlock.clear();
    }

    //resolve tinyurl to underlying sharepoint url and to get session cookies
    let mut clock = Stopwatch::start_new();
    let response = ctx.client.get(url.clone()).send()?;
    clock.stop();
    println!("v1 time: {}ms",clock.elapsed_ms());
    let once_url = response.url().as_str();
    let mut session_cookies = response.cookies();

    let download_url: Url = once_url.replace("onedrive.aspx", "download.aspx").replace("?id=","?SourceUrl=").parse()?;

    //FedAuth session key is needed from the original link for the direct download link to work. 
    match session_cookies.find(|c| c.name()=="FedAuth"){
        Some(_) => {},
        None => {return Err(anyhow!("Did not find FedAuth session cookie")); }
    }

    //redirect to download 
    let response = ctx.client.head(download_url.clone()).send()?;
    if !response.status().is_success(){
        return Err(anyhow!("failed to fetch url {} - HTTP error {}: {}",&download_url,response.status().as_str(),response.text()?));
    }
    let file_size = match response.headers().get(reqwest::header::CONTENT_LENGTH) {
        Some(sz) => sz.to_str()?.parse()?,
        None => return Err(anyhow!("file download size unknown"))
    };

    let etag = match response.headers().get(reqwest::header::ETAG){
        Some(e) => e.to_str()?.to_string().replace("\"", ""),
        None => return Err(anyhow!("failed to get ETag for download file"))
    };

    //filename
    let filename = match response.headers().get(reqwest::header::CONTENT_DISPOSITION) {
        None => return Err(anyhow!("failed to get filename")),
        Some(v) => v.to_str()?.split("filename=").last().ok_or(anyhow!("failed to get filename"))?.replace("\"", "")
    };
    return Ok(DownloadInfo{etag: etag, sessionUrl: url.clone(), downloadUrl: download_url, filename: filename, fileSize: file_size})
}

struct ClientCtx {
    client: Client,
    jar: Arc<CookieStoreRwLock>
}

mod msgraph;
mod dirhash;




fn main() -> Result<(), Error> {
    //dirhash
    let args: Vec<String> = env::args().collect();
    let p = Path::new(args[1].as_str());
    dirhash::build_dir_manifest(&p,&Path::new("CAC-config/manifest.json"));
    return Ok(());

    //let path = Path::new("D:\\SteamLibrary\\steamapps\\common\\Arma 3\\Mods\\@ace");
    //let path = Path::new("D:\\SteamLibrary\\steamapps\\common\\Arma 3\\Mods");
    //let path = Path::new("D:\\SteamLibrary\\steamapps\\common\\Arma 3\\Mods\\@CUPTerrainsMaps"); 
    
    // let mut clock = Stopwatch::start_new();
    // println!("{} hash: {}",path.to_string_lossy(),dirhash::hash_directory(path)?.to_string());
    // clock.stop();
    // println!("hash time: {}ms",clock.elapsed_ms());

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("user-agent",USER_AGENT.parse()?);

    let jar = Arc::new(CookieStoreRwLock::new(CookieStore::new(None)));
    let clientCtx = ClientCtx{
        client: reqwest::blocking::Client::builder()
        .cookie_provider(jar.clone())
        .default_headers(headers)
        .redirect(Policy::limited(10))
        .use_native_tls()
        .build()?,
        jar: jar
    };
  
    let url = "https://tinyurl.com/4hznh2sa";
    let url = "https://0jz1q-my.sharepoint.com/:u:/g/personal/brenner650_0jz1q_onmicrosoft_com/ER8foAfRn8BPp_AqMlDVJNoBlOn17yHic_ixZNbwKWOlng?e=gGBEge";

    let info = get_download_info(&clientCtx,Url::from_str(url)?,&Path::new("./tmp/"))?;
    println!("etag: {}",info.etag);
    println!("filename: {}",info.filename);

    // let token = msgraph::login(&clientCtx.client)?;
    // while(true){
    //     msgraph::getSharedDriveItem(&clientCtx.client, token.as_str(), url)?;
    // }
    
    Ok(())
}


