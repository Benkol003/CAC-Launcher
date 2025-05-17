#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

/// utilities for interacting with the [msgraph api](https://learn.microsoft.com/en-us/graph/use-the-api) for downloading content.
/// Note: the application needs File.ReadWrite permissions granted for it to work in entra admin center. these can only be granted by an admin.
pub mod msgraph;
///define the app's secrets here such as the msgraph key.
pub mod secrets;
///utilities for verifying downloaded mods.
pub mod dirhash;

use std::{sync::Mutex,default, env, fmt::Debug, io::Write, path::Path, str::FromStr, sync::Arc, time::SystemTime, usize};
use anyhow::{anyhow,Error};
use base64::display;
use reqwest::{blocking::{Client, Request, Response}, header, redirect::Policy,Url};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex, CookieStoreRwLock};
use stopwatch::Stopwatch;
use tokio_util::sync::CancellationToken;

use jwalk::WalkDir;

//app will be blocked without this. reccommend using a browser user agent string to prevent rate limiting.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";
pub const PROGRESS_STYLE: &str = "{spinner} {msg:.green.bold} {percent}% {decimal_bytes}/{decimal_total_bytes} [{decimal_bytes_per_sec}], Elapsed: {elapsed}, ETA: {eta}";

pub struct DownloadInfo {
        pub sessionUrl: Url,
        pub downloadUrl: Url,
        pub filename: String,
        pub fileSize: usize,
        pub etag: String,
}

//download until finished or canceelled. If file exists at path then resumes a partial download.
//assume client is in a state that it has the neccesary cookies to use the download url
pub async fn download_file(info: DownloadInfo,path: &Path,canceller: CancellationToken) -> Result<(),Error> {
    let do_partial = Path::exists(path);

    return Ok(());
}

//using reqwest
pub fn get_download_info(ctx: &ClientCtx,url: Url,dir: &Path) -> Result<DownloadInfo,Error> {

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

pub struct ClientCtx {
    pub client: Client,
    pub jar: Arc<CookieStoreRwLock>
}

pub fn build_client_ctx() -> Result<ClientCtx,Error> {
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
    Ok(clientCtx)
}