#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]
#![allow(warnings)]

use anyhow::{anyhow, Error};
use futures_util::TryStreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use log::warn;
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::Deserialize;
use serde_json::map::Entry;
use tokio::sync::{Mutex, RwLock};
use std::clone;
use std::fmt::Display;
use std::time::SystemTime;
use std::{
    collections::HashMap,
    fmt::Debug,
    fs::OpenOptions,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    pin::{self, Pin},
    task::Context,
    time::Duration,
};
use stopwatch::Stopwatch;
use tokio::{
    io::{AsyncRead, AsyncReadExt, BufReader},
    select,
    time::sleep,
};
use tokio_util::{
    io::{poll_read_buf, StreamReader},
    sync::CancellationToken,
};
use urlencoding;

use base64::{
    self,
    prelude::{BASE64_STANDARD_NO_PAD, BASE64_URL_SAFE_NO_PAD},
    read, Engine,
};

use reqwest::{
    get,
    header::{self, HeaderMap, InvalidHeaderValue, CONTENT_LENGTH, CONTENT_TYPE, HOST, RANGE},
    Client, Request, Response, StatusCode, Url, Version,
};

use crate::download::download_file;
use crate::{PROGRESS_STYLE_DOWNLOAD, TIMEOUT, final_url, secrets};

const TENANT_ID: &str = "e49b9dd1-c246-47a2-8487-0d3d6b6a5ae3";
const APP_CLIENT_ID: &str = "19bf2428-3178-416d-a169-a62ba28c23fb";
const MSAPI_URL: &str = "https://graph.microsoft.com/v1.0/";
//const MSGPRAPH_KEY - place in secrets.rs

///[msgraph reference](https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token)
#[derive(Deserialize,Clone,Debug)]
pub struct Token {
    pub token_type: String,
    pub expires_in: usize,
    pub ext_expires_in: usize,
    pub access_token: String,

    #[serde(skip, default = "std::time::SystemTime::now")]
    pub obtained_at: SystemTime
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
    /// globally unique id for the drive item.
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
    Folder {
        child_count: usize,
    },
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http)
async fn get_encoded_sharing_url(client: &Client, url: Url) -> Result<String, Error> {
    let final_url = final_url(client.clone(),url).await?.to_string();
    warn!("final url: {}",final_url);
    return Ok(format!("u!{}", BASE64_URL_SAFE_NO_PAD.encode(final_url)));
}

#[derive(thiserror::Error, Debug)]
pub enum MsGraphError {
    #[error("{0}")]
    GenericError(#[from] Box<dyn std::error::Error + Send + Sync>),
    #[error("{0}")]
    ReqwestError(#[from] reqwest::Error),
}

impl From<InvalidHeaderValue> for MsGraphError {
    fn from(value: InvalidHeaderValue) -> Self {
        Self::GenericError(value.into())
    }
}

impl From<anyhow::Error> for MsGraphError {
    fn from(value: anyhow::Error) -> Self {
        Self::GenericError(value.into())
    }
}

pub fn is_sharepoint_link(url: &str) -> Result<bool, Error> {
        let auth_regex = Regex::new("^(?:[\\w-]+\\.)+sharepoint.com$")?;
        return Ok(auth_regex.is_match(url));
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http)
pub async fn get_shared_drive_item(
    client: Client,
    url: Url,
) -> Result<SharedDriveItem, MsGraphError> {
    let client = reqwest::Client::new(); //TODO use client ctx instead
    //let mut params = HashMap::new();

    let share_id = get_encoded_sharing_url(&client, url).await?;
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token(&client).await?.access_token).parse()?);
    headers.append(header::CONTENT_TYPE, "application/json".parse()?);
    headers.append("prefer", "redeemSharingLink".parse()?);
    let mut response = client
        .get(format!("{}shares/{}/driveItem", MSAPI_URL, share_id))
        .headers(headers)
        .timeout(TIMEOUT)
        .send()
        .await?;
    if response.status().as_u16() != 200 {
        return Err(anyhow!(
            "http error - code {}, text: {}",
            response.status().as_u16(),
            response.text().await?
        )
        .into());
    }

    let mut ret: SharedDriveItem = response.json().await?;
    ret.share_id = share_id;
    return Ok(ret);
}

pub struct DownloadRequest {
    dest: PathBuf,
    /// path for the temporary file used for partial downloads
    tmp_dest: PathBuf,
    request: Request,
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/driveitem-get-content?view=graph-rest-1.0&tabs=http)
/// # Returns
/// path to the temporary file downloaded, or None if cancelled, or an Error.
/// downloads will be resumed later after a cancel if you attempt to download the same drive item to the same destination folder.
/// TODO: if there is a folder with the same name as the download file...
pub async fn download_item(
    client: Client,
    item: SharedDriveItem,
    dest_folder: String,
    progress: &mut ProgressBar,
    cancel: CancellationToken,
) -> Result<Option<PathBuf>, Error> {
    progress.set_style(ProgressStyle::with_template(PROGRESS_STYLE_DOWNLOAD)?);
    let dest_folder = Path::new(dest_folder.as_str());
    std::fs::create_dir_all(dest_folder)?;

    warn!("SharedDriveItem::name  = {}",item.name);

    /////// build msgraph request /////////

    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token(&client).await?.access_token).parse()?);
    let dest_url = Url::parse(format!("{}shares/{}/driveItem/content",MSAPI_URL, item.share_id).as_str())?;

    
    download_file(client,item.name,dest_url,Some(headers),dest_folder,progress,item.id.as_str(),cancel).await
}

static TOKEN: Mutex<Option<Token>> = Mutex::const_new(None);


//returns a globally maintained instance of the app token, requests a new token 
//if the new one has expired.
pub async fn token(client: &Client) -> Result<Token, Error> {
    let mut guard = TOKEN.lock().await;
    let token = match guard.as_ref() {
        None => {
            let t = obtain_token(client).await?; *guard = Some(t.clone()); t
        },
        Some(t) => {
            let lived = SystemTime::now().duration_since(t.obtained_at)?;
            //refresh token early to prevent race condition
            if (lived.as_secs() as usize) > (t.expires_in - 60) {
                let t = obtain_token(client).await?; *guard = Some(t.clone()); t
            } else {
                t.clone()
            }
        }
    };
    Ok(token)
}


///[msgraph reference](https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token)\
/// returns an access that can be used with other MSGraph API endpoints
pub async fn obtain_token(client: &Client) -> Result<Token, Error> {
    let mut params = HashMap::new();
    params.insert("client_id", APP_CLIENT_ID);
    params.insert("scope", "https://graph.microsoft.com/.default");
    params.insert("client_secret", secrets::MSGRAPH_KEY);
    params.insert("grant_type", "client_credentials");

    let response = client
        .post(format!(
            "https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token"
        ))
        .header(HOST, "login.microsoftonline.com")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .form(&params)
        .timeout(TIMEOUT)
        .send()
        .await?;

    if response.status().is_success() {
        let json: Token = response.json().await?;
        return Ok(json);
    } else {
        return Err(anyhow!(
            "failed to get access token. status code: {}",
            response.status().as_str()
        ));
    }
}
