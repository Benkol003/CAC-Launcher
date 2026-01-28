use std::{cell::RefCell, fs::OpenOptions, io::{Seek, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

use anyhow::{Error, anyhow};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use futures_util::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use reqwest::{Client, Request, StatusCode, Url, header::{self, HeaderMap}};
//use sha2::{Digest, Sha256, Sha512};
use tokio::{io::AsyncReadExt, time::sleep};
use tokio_util::{io::StreamReader, sync::CancellationToken};

use crate::{ClientCtx, PROGRESS_STYLE_DOWNLOAD, PROGRESS_STYLE_MESSAGE, TIMEOUT, configs::*, final_url, msgraph::{self, MsGraphError}, unzip};

//TODO replace remove_dir_all with this
pub fn remove_path(path: &Path) -> std::io::Result<()> {
    let metadata =     std::fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        return std::fs::remove_dir_all(path);
    }else if metadata.is_file() || metadata.is_symlink() {
        return std::fs::remove_file(path);
    }
    Ok(())
}

//wraps di so can cancel remaining items if an error occurs.
pub async fn download_items(items: Vec<String>,mut progress: ProgressBar, title_buf: Arc<Mutex<String>>, finish: CancellationToken) -> Result<bool,Error> {
    let ret = di(items,&mut progress,title_buf,&finish).await;
    finish.cancel();
    ret
}

pub fn fetcH_file_info(client: Client,
    headers: Option<HeaderMap>,
    cancel: CancellationToken
    ) -> Result<(),Error> {
        Err(anyhow!("not impl"))
    }


/// generic file download for msgraph or normal links.
/// alternative_tmp_id: id to use other than the url e.g. sharepoint drive + item ID for temp partial downloads
/// # Returns
/// path to the temporary file (partial or full) downloaded, or None if cancelled, or an Error.
/// TODO !! sometimes get error decoding response body (pretty sure this is just internet disconnect)
/// TODO split out to get download info, can use info from msgraph instead
/// TODO: auth via github REST API + auth token for download link
/// https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api
pub async fn download_file(client: Client,
    display_name: String,
    dest_url: Url, 
    headers: Option<HeaderMap>, 
    dest_folder: &Path, 
    progress: &mut ProgressBar, 
    //unique ID for the file / url to download. This is base64 encoded along with the eTag to produce the temp file ID hash.
    //using a hasher to produce a consistent file name length <255.
    tmp_id: &str,
    cancel: CancellationToken
    ) -> Result<Option<PathBuf>, Error> {
    progress.set_style(ProgressStyle::with_template(PROGRESS_STYLE_DOWNLOAD)?);
    std::fs::create_dir_all(dest_folder)?;

    let headers = headers.unwrap_or(HeaderMap::new());
    
    let mut head_headers = headers.clone();
    //msgraph links will reject a HEAD request, so do GET + drop
    let response = client.get(dest_url.clone()).headers(head_headers.clone()).timeout(TIMEOUT).send().await?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "download URL HTTP error: {}",
            response.status().as_str()
        ));
    }
    let item_size: u64 = response.headers().get(header::CONTENT_LENGTH)
    .ok_or(anyhow!("Content-Length file size not in response header for {}",dest_url))?.to_str()?.parse()?;

    //construct filename from url and eTag
    let etag: String = response.headers().get(header::ETAG).ok_or(anyhow!("no eTag in response header for {}",dest_url))?.to_str()?.to_owned();

    // let mut hasher = Sha512::new();
    // hasher.write(dest_url.as_str().as_bytes());
    // hasher.write(etag.as_bytes());
    // let fname: String =  BASE64_URL_SAFE_NO_PAD.encode(hasher.finalize());
    let fname = match response.headers().get(header::CONTENT_DISPOSITION) {
        None => return Err(anyhow!("failed to get filename")),
        //TODO add support for filename*='<encoding>'<filename>
        Some(v) => v.to_str()?.split("filename=").last().ok_or(anyhow!("failed to get filename"))?.replace("\"", "")
    };

    //force close the connection
    drop(response);


    //TODO
    //also pass option<config> in for tests or edit return
    //let mut tmp_manifest = CACDownloadManifest::read()?;
    //tmp_manifest.0.insert(fname.clone(), TmpDownloadID{id: tmp_id.to_string(), etag: etag.clone()});

    
    let dest_path = dest_folder.join(fname);
    warn!("dest_path: {}",dest_path.display());


    warn!("downloading {}, size={}",dest_url,item_size);

    //check if file exists so can resume partial downloads
    let mut file: std::fs::File;
    let mut start = 0;
    if std::fs::exists(&dest_path)? {
        file = OpenOptions::new()
            .append(true)
            .write(true)
            .open(&dest_path)?;
        let metadata = file.metadata()?;
        start = metadata.len();
    } else {
        file = std::fs::File::create(&dest_path)?;
    }

    if start >= item_size {
        return Ok(Some(dest_path));
        progress.set_message(format!("Downloading {}", display_name));
    }

    let mut get_headers = headers;
    if (start != 0) {
        get_headers.append(header::RANGE, format!("bytes={start}-").parse()?);
    }

    progress.set_length(item_size);
    progress.set_message(format!("Downloading {}", display_name));

    let response = client.get(dest_url).headers(get_headers)
    //we cant disable the timeout by passing None
    //https://github.com/seanmonstar/reqwest/issues/1366
    //unset timeout as dont know how long large files will take. instead timeout for recieving data blocks
    .timeout(Duration::MAX) 
    .send().await?;

    if (!response.status().is_success()) {
        return Err(anyhow!(
            "download URL HTTP error: {}",
            response.status().as_str()
        ));
    }

    if start != 0 && response.status() != StatusCode::PARTIAL_CONTENT {
        warn!("didnt recieve '206 Partial Content' response when trying to do a partial download / range request.");
        file.set_len(0);
        file.seek(std::io::SeekFrom::Start(0));
    }

    //BufReader wont read more than 16KB anyway most likely due to max MTU size
    const BLOCK_SIZE: usize = 16 * 1024;
    let mut buf = Box::new([0; BLOCK_SIZE]);
    let mut readBytes: usize;
    let reader = response.bytes_stream();
    let mut reader = StreamReader::new(reader.map_err(|e| std::io::Error::other(e)));
    progress.set_position(start);
    while (true) {
        tokio::select! {
            _ = cancel.cancelled() => {
                return Ok(None);
            }
            readBytes = reader.read(&mut buf[..BLOCK_SIZE]) => {
                let readBytes = readBytes?;
                if(readBytes==0) {break;}
                file.write(&buf[..readBytes])?;
                progress.inc(readBytes as u64);
            }
            _ = sleep(TIMEOUT) => {
                return Err(anyhow!("download timed out"));
            }
        };
    }
    progress.reset(); //TODO should be calling finish_and_clear() and then creating a new progress bar - make a custom progress indicator
    return Ok(Some(dest_path));
}

async fn di(items: Vec<String>,progress: &mut ProgressBar, title_buf: Arc<Mutex<String>>, finish: &CancellationToken) -> Result<bool,Error>{
            let mut config = CACConfig::read()?;
            let client_ctx = ClientCtx::build()?; //TODO initialise elsewhere
            let token = msgraph::login(&client_ctx.client).await?;

            //TODO indicate on n/total items

            //TODO: support for optional mods in other mods: sort items to copy out optional mods inside other mods after their parent
            //these wont be added to the update list in the config list atm if the parent needs updating - add logic to handle this,
            //and to update both if either are in the update list 

            let content = CACContent::read()?;
            let content_map =  content.content_map();
            for (i,item) in items.iter().enumerate() {
                {
                    let mut lock = title_buf.lock().unwrap();
                    *lock = format!("{}/{}",i,items.len());
                }

                let links  =content_map.get(&item.to_string()).unwrap();
                
                let mut files: Vec<PathBuf> = Vec::new();
                for link in links.into_iter() {

                    match Url::parse(link) {
                        Ok(link_url) => {
                                    
                            let final_url = final_url(client_ctx.client.clone(), link_url.clone()).await?;
                            
                            progress.set_message(" Fetching info... ");
                            warn!("link: {}",link);
                            let optfile = match msgraph::is_sharepoint_link(&final_url.authority())? {
                                true => {
                                    let item = msgraph::get_shared_drive_item(client_ctx.client.clone(), token.clone(),link_url).await?;
                                    msgraph::download_item(client_ctx.client.clone(), token.clone(),item, TMP_FOLDER.display().to_string(), progress, finish.clone()).await?
                                    
                                }
                                false => {
                                    //generic link download
                                    download_file(client_ctx.client.clone(),item.clone(),final_url.clone(),None,TMP_FOLDER.as_path(),progress,
                                                        //TODO tmp id ends up too long on windows using final dest url, errors out at fs::exists
                                    final_url.as_str(),finish.clone()).await?
                                }
                            };
                            let file = match optfile {
                                Some(f) => f,
                                None => return Ok(false)
                            };
                            files.push(file);                        
                        }
                        //not a valid url. assume is a reference to an optional mod in another mod
                        //or whatever you wanna add later
                        Err(e) => {
                            //TODO not impl
                            return Err(e.into());
                        }

                    }
                }
                let dest = match item.starts_with("@"){
                    false => {PathBuf::from(&config.arma_path).parent().unwrap().display().to_string()}
                    true => {config.absolute_mod_dir()?.display().to_string()}
                };

                //TODO delete the old folder before unzipping if present
                //need logic to make sure files are top level as if unzipping to .
                //will unzip into e.g../@ace
                //TODO double check getting archive .000

                //TODO temp fix
                let mut dest_folder = Path::new(&dest).join(item);
                warn!("removing {} before unzip", dest_folder.display());
                if dest_folder.exists() {
                    if dest_folder.is_dir() {
                        std::fs::remove_dir_all(&dest_folder)?;
                    }else {
                        return Err(anyhow!("refusing to remove '{}' as not a folder",dest_folder.display()));
                    }
                }

                unzip(files.get(0).unwrap().to_str().unwrap(),&dest,Some(progress))?;

                progress.set_style(ProgressStyle::with_template(PROGRESS_STYLE_MESSAGE)?);
                progress.set_message(" cleaning up..."); progress.set_length(1); progress.set_position(0);
                for f in files {
                    std::fs::remove_file(f)?;
                }

                config.pending_updates.remove(item);
                config.save()?;
            }
            Ok(true)
        }