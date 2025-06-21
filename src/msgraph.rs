#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]
#![allow(warnings)]

use anyhow::{ anyhow, Error };
use futures_util::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use serde_json::map::Entry;
use serde::{ Deserialize };
use tokio::{io::{AsyncRead, AsyncReadExt, BufReader}, select, time::sleep};
use tokio_util::{io::{poll_read_buf, StreamReader}, sync::CancellationToken};
use std::{ collections::HashMap, fmt::Debug, fs::OpenOptions, io::{Read, Seek, Write}, path::{Path, PathBuf}, pin::{self, Pin}, task::Context, time::Duration };
use stopwatch::Stopwatch;
use urlencoding;

use base64::{ self, prelude::{ BASE64_STANDARD_NO_PAD, BASE64_URL_SAFE_NO_PAD }, read, Engine };

use reqwest::{
    get, header::{ self, HeaderMap, CONTENT_LENGTH, CONTENT_TYPE, HOST, RANGE }, Client, StatusCode, Url, Version
};

use crate::{secrets, PROGRESS_STYLE, TIMEOUT};

const TENANT_ID: &str = "4fd01353-8fd7-4a18-a3a1-7cd70f528afa";
const APP_CLIENT_ID: &str = "9ecaa0e8-9caf-4f49-94e8-8430bbf57486";
const MSAPI_URL: &str = "https://graph.microsoft.com/v1.0/";
//const MSGPRAPH_KEY - place in secrets.rs

///[msgraph reference](https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token)
#[derive(Deserialize)] //TODO remove?
pub struct TokenResponse {
    pub token_type: String,
    pub expires_in: usize,
    pub ext_expires_in: usize,
    pub access_token: String,
}


/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/resources/driveitem?view=graph-rest-1.0)
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SharedDriveItem {
    /// the encoded sharing url for making new requests. This is not part of the msgraph DriveItem json response.
    #[serde(skip)]
    pub share_id: String,
    pub name: String,
    pub size: u64,
    pub id: String,
    pub cTag: String,

    #[serde(flatten)]
    pub item: FsEntryType,
}

// #[derive(Deserialize, Debug)]
// #[serde(rename_all = "camelCase")]
// pub struct FileItem {
//     pub quick_xor_hash: String,
// }

// #[derive(Deserialize, Debug)]
// pub struct FolderItem {
//     pub child_count: usize,
// }

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Hashes {
    pub quick_xor_hash: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FsEntryType {
    File {
        hashes: Hashes,
    },
    #[serde(rename_all = "camelCase")]
    Folder {child_count: usize},
}


/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http)
async fn get_encoded_sharing_url(client: &Client, url: &str) -> Result<String, Error> {
    let response = client.get(url).send().await?;
    let final_url = response.url().as_str();
    return Ok(format!("u!{}", BASE64_URL_SAFE_NO_PAD.encode(final_url)));
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http)
pub async fn get_shared_drive_item(
    client: Client,
    token: String,
    url: String
) -> Result<SharedDriveItem, Error> {
    let client = reqwest::Client::new();
    //let mut params = HashMap::new();

    let share_id = get_encoded_sharing_url(&client,&url).await?;
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token).parse()?);
    headers.append(header::CONTENT_TYPE, "application/json".parse()?);
    headers.append("prefer", "redeemSharingLink".parse()?);
    let mut response = client
        .get(format!("{}shares/{}/driveItem", MSAPI_URL, share_id))
        .headers(headers)
        .send().await?;
    if response.status().as_u16() != 200 {
        return Err(
            anyhow!("http error - code {}, text: {}", response.status().as_u16(), response.text().await?)
        );
    }

    let mut ret: SharedDriveItem = response.json().await?;
    ret.share_id = share_id;
    return Ok(ret);
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/driveitem-get-content?view=graph-rest-1.0&tabs=http)
/// # Returns
/// path to the temporary file downloaded, or an Error. Will return an error if the download is cancelled.
/// downloads will be resumed later after a cancel if you attempt to download the same drive item to the same destination folder.
/// TODO: if there is a folder with the same name as the download file...
pub async fn download_item(client: Client, token: String, item: SharedDriveItem,dest_folder: String,progress: &mut ProgressBar, cancel: CancellationToken) -> Result<PathBuf, Error> {
    let dest_folder = Path::new(dest_folder.as_str());
    std::fs::create_dir_all(dest_folder)?;

    //use the drive item ID + the cTag (content tag) as the download file name;
    //for partial file downloads to differentiate between files of the same name
    //and for partial download / cache invalidation
    let fname = format!("id_{}_eTag-b64_{}.tmp",item.id,BASE64_URL_SAFE_NO_PAD.encode(item.cTag));

    let dest_path = dest_folder.join(fname);

    //check if file exists so can resume partial downloads
    let mut file: std::fs::File;
    let mut start = 0;
    if std::fs::exists(&dest_path)?{
        file = OpenOptions::new().append(true).write(true).open(&dest_path)?;
        let metadata = file.metadata()?;
        start = metadata.len();
    }else{
        file = std::fs::File::create(&dest_path)?;
    }

    if start>=item.size{
        return Ok(dest_path);
    }

    progress.set_length(item.size as u64);
    progress.set_message(format!("Downloading {}",item.name)); 

    //cd "" == cd "./"
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token).parse()?);
    if(start!=0){
        headers.append(RANGE, format!("bytes={start}-").parse()?);
    }

    let mut response = client
        .get(format!("{}shares/{}/driveItem/content", MSAPI_URL, item.share_id))
        .headers(headers)
        .send().await?;

    if(!response.status().is_success()) {
        return Err(anyhow!("download URL HTTP error: {}",response.status().as_str()));
    }

    if start!=0 && response.status()!=StatusCode::PARTIAL_CONTENT {
        warn!("didnt recieve '206 Partial Content' response when trying to do a partial download / range request.");
        file.set_len(0);
        file.seek(std::io::SeekFrom::Start(0));
    }
 
    //BufReader wont read more than 16KB anyway most likely due to max MTU size
    const BLOCK_SIZE: usize = 16*1024;
    let mut buf  = Box::new([0;BLOCK_SIZE]);
    let mut readBytes : usize;
    let reader = response.bytes_stream();
    let mut reader = StreamReader::new(reader.map_err(|e| std::io::Error::other(e)));
    progress.set_position(start);
    while(true){
        tokio::select! {
            _ = cancel.cancelled() => {
                return Err(anyhow!("download cancelled")); //TODO this isnt really an error...
            }
            readBytes = reader.read(&mut buf[..BLOCK_SIZE]) => {
                let readBytes = readBytes?;
                if(readBytes==0) {break;}
                file.write(&buf[..readBytes])?;
                progress.inc(readBytes as u64);
            }
        _ = sleep(TIMEOUT)=> {
            return Err(anyhow!("download connection timed out."));
        }
        };

    }
    progress.finish_and_clear();
    Ok(dest_path)
}

///[msgraph reference](https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token)\
/// returns an access that can be used with other MSGraph API endpoints
pub async fn login(client: &Client) -> Result<String, Error> {
    let mut params = HashMap::new();
    params.insert("client_id", APP_CLIENT_ID);
    params.insert("scope", "https://graph.microsoft.com/.default");
    params.insert("client_secret", secrets::MSGRAPH_KEY);
    params.insert("grant_type", "client_credentials");

    let response = client
        .post(format!("https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token"))
        .header(HOST, "login.microsoftonline.com")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&params)
        .send().await?;

    if response.status().is_success() {
        let json: TokenResponse = response.json().await?;
        return Ok(json.access_token);
    } else {
        return Err(
            anyhow!("failed to get access token. status code: {}", response.status().as_str())
        );
    }
}
