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
pub mod UI;
pub mod servers;

use std::{default, env, fmt::Debug, fs::{remove_file, File}, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, str::FromStr, sync::{Arc, Mutex}, time::SystemTime, usize};
use anyhow::{anyhow,Error};
use base64::display;
use indicatif::{ProgressBar, ProgressStyle};
use msgraph::SharedDriveItem;
use once_cell::unsync::{Lazy, OnceCell};
use regex::{bytes::Match, Regex};
use reqwest::{Client, Request, Response, header, redirect::Policy,Url};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex, CookieStoreRwLock};
use stopwatch::Stopwatch;
use tokio_util::sync::CancellationToken;

use jwalk::WalkDir;

//app will be blocked without this. reccommend using a browser user agent string to prevent rate limiting.
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:135.0) Gecko/20100101 Firefox/135.0";
pub const PROGRESS_STYLE: &str = "{spinner} {msg:.green.bold} {percent}% {decimal_bytes}/{decimal_total_bytes} [{decimal_bytes_per_sec}], Elapsed: {elapsed}, ETA: {eta}";
pub const CONFIG_FOLDER: Lazy<PathBuf> = Lazy::new(|| {
    PathBuf::from("CAC-Config")
});
pub const TMP_FOLDER: Lazy<PathBuf> = Lazy::new(|| {
    CONFIG_FOLDER.join("tmp")
});

pub static Z7_EXE: &[u8] = include_bytes!("7za.exe");

pub struct FileAutoDeleter(PathBuf);

impl FileAutoDeleter {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        FileAutoDeleter(
            path.as_ref().to_path_buf()
        )
    }
}

impl Drop for FileAutoDeleter {
 fn drop(&mut self) {
    std::fs::remove_file(&self.0).unwrap();
 }
}

pub fn force_create_dir(path: &PathBuf) -> Result<(),Error>{
    if !std::fs::exists(path)? {
        std::fs::create_dir(path);
    }else if !std::fs::metadata(path)?.is_dir(){
        std::fs::remove_file(path);
        std::fs::create_dir(path);
    }
    Ok(())
}


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
pub async fn get_download_info(ctx: &ClientCtx,url: Url,dir: &Path) -> Result<DownloadInfo,Error> {

    //make sure to clear out FedAuth cookie so it doesnt break stuff
    {
        let mut jarlock = ctx.jar.write().unwrap();
        jarlock.clear();
    }

    //resolve tinyurl to underlying sharepoint url and to get session cookies
    let mut clock = Stopwatch::start_new();
    let response = ctx.client.get(url.clone()).send().await?;
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
    let response = ctx.client.head(download_url.clone()).send().await?;
    if !response.status().is_success(){
        return Err(anyhow!("failed to fetch url {} - HTTP error {}: {}",&download_url,response.status().as_str(),response.text().await?));
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
        client: reqwest::Client::builder()
        .cookie_provider(jar.clone())
        .default_headers(headers)
        .redirect(Policy::limited(10))
        .use_native_tls()
        .build()?,
        jar: jar
    };
    Ok(clientCtx)
}

/// Given a vec of drive items, will group partial archives by their full archive name (without the .nnn extension). Single archives are just a vec of one element
pub fn group_drive_item_archives(drive_items: Vec<SharedDriveItem>) -> Result<Vec<(String, Vec<SharedDriveItem>)>,Error> {
    let mut items: Vec<(String, Vec<SharedDriveItem>)> = Vec::new();

    for i in drive_items {
        let (basename, ext) = i.name.rsplit_once('.').ok_or(anyhow!("failed to rsplit filename to get extension"))?;
        let re_partial_ext = Regex::new("^[0-9]{3}$")?;
        if re_partial_ext.is_match(ext){
            //partial archive grouping 
            match items.iter().position(|j| {j.0 == basename}){
                None => {
                    items.push((basename.to_string(),vec!(i)));
                }
                Some(idx) => {
                    items[idx].1.push(i);
                }
            }
        }else{
            items.push((basename.to_string(),vec!(i)));
        }
    }
    Ok(items)
}

pub fn unzip(fname: &str,dest: &str, progress: &mut ProgressBar) -> Result<String,Error> {
    let mut z7_stderr_log: Vec<u8> = Vec::new();

    let o_arg = format!("-o{}",dest);
    let args = [
            "e",
            "-y",
            o_arg.as_str(),
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
        let mut reader = BufReader::new(z7_run.stderr.take().ok_or(anyhow!("failed to get stderr to running process"))?);
        let mut buf = Vec::new();
        while z7_run.try_wait()?.is_none() {
            buf.clear();

            //process 7zip's progress output
            let r = reader.read_until('\r' as u8,&mut buf)?;

            z7_stderr_log.extend(buf.clone()); //TODO RM
            
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
                    progress.set_message(s.1.to_string());
                }
            }
            let pc = pc.to_string(); let pci: u64 = pc.trim().parse()?;
            progress.set_position(pci);
            if r==0 {
                break;
            }
        }
        progress.finish_and_clear();

        let error = z7_run.wait()?;
        if !error.success(){
            let mut reader = BufReader::new(z7_run.stdout.ok_or(anyhow!("failed to get stdout to running process"))?);
            let mut z7log = String::new();
            reader.read_to_string(&mut z7log)?;
            let mut f = std::fs::File::create("7z.log")?;
            f.write(z7log.as_bytes())?;
            
            f.write(&z7_stderr_log)?;

            return Err(anyhow!("failed to extract {} (see 7z.log)",fname));
        }

        //return the folder name we extracted to
        //will get the folder name from the archive name. also works with multipart archives ending in .nnn
        let regex = Regex::new(r#"^(.*?)\.(?:zip|7z)(?:\.\d{3})?$"#).unwrap();
        Ok(regex.captures(fname).unwrap().get(1).unwrap().as_str().to_string())
}

pub fn launch_steam() -> Result<(),Error> {
    use sysinfo::ProcessRefreshKind;
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

    let steam: PathBuf;
    #[cfg(windows)]
    {
        let hklm= RegKey::predef(HKEY_LOCAL_MACHINE);
        let steam_hkey: String = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Valve\\Steam").map_err(|_| anyhow!("steam not installed"))?.get_value("InstallPath")?;
        steam = PathBuf::from(steam_hkey).join("steam.exe");
    }

    #[cfg(linux)]
    {
        steam = "steam.exe".into();
    }

    Command::new(steam).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
    let mut sys = sysinfo::System::new();
    sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());

    for (_, process) in sys.processes() {
        if process.name()=="steam.exe"{
            return Ok(());
        }
    }
    Err(anyhow!("steam failed to launch"))
}