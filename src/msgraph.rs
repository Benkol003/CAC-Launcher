#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]
#![allow(warnings)]

use anyhow::{ anyhow, Error };
use futures_util::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::map::Entry;
use serde::{ Deserialize };
use tokio::io::{AsyncReadExt, BufReader};
use tokio_util::io::StreamReader;
use std::{ collections::HashMap, fmt::Debug, io::{Read, Write}, path::Path };
use stopwatch::Stopwatch;
use urlencoding;

use base64::{ self, prelude::{ BASE64_STANDARD_NO_PAD, BASE64_URL_SAFE_NO_PAD }, read, Engine };

use reqwest::{
    {get, Client },
    header::{ self, HeaderMap, CONTENT_TYPE, HOST },
    Url,
    Version,
};

use crate::{secrets, PROGRESS_STYLE};

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
    pub size: usize,
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
pub async fn download_item(client: Client, token: String, item: SharedDriveItem,dest_folder: String) -> Result<(), Error> {
    let dest_folder = Path::new(dest_folder.as_str());
    std::fs::create_dir_all(dest_folder)?;
    let mut file =  std::fs::File::create(dest_folder.join(item.name.clone()))?;
    let progress = ProgressBar::new(item.size as u64).with_style(ProgressStyle::with_template(PROGRESS_STYLE)?);//TODO static assert usize::MAX<= u64::MAX
    progress.set_message(format!("Downloading {}",item.name)); 

    //cd "" == cd "./"
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token).parse()?);

    // TODO test: does this block until the entire file has been downloaded
    let mut response = client
        .get(format!("{}shares/{}/driveItem/content", MSAPI_URL, item.share_id))
        .headers(headers)
        .send().await?;

    if(!response.status().is_success()) {
        return Err(anyhow!("download URL HTTP error: {}",response.status().as_str()));
    }
 
    //BufReader wont read more than 16KB anyway most likely due to max MTU size
    const BLOCK_SIZE: usize = 16*1024;
    let mut buf : [u8; BLOCK_SIZE] = [0;BLOCK_SIZE];
    let mut readBytes : usize;
    let reader = response.bytes_stream();

    //TODO: profile between using stream directly to file or use buffered reader
    let reader = StreamReader::new(reader.map_err(|e| std::io::Error::other(e)));
    let mut reader = BufReader::with_capacity(BLOCK_SIZE,reader);
    while(true){
        readBytes = reader.read(&mut buf).await?;
        if(readBytes==0) {break;}
        file.write(&buf[..readBytes])?;
        progress.inc(readBytes as u64);
    }
    progress.finish_and_clear();
    Ok(())
}

///[msgraph reference](https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token)\
///TODO: switch to using a certificate
pub async fn login(client: &Client) -> Result<String, Error> {
    //get an access token
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
