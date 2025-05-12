#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]
#![allow(warnings)]

use anyhow::{ anyhow, Error };
use serde_json::map::Entry;
use serde::{ Deserialize };
use std::{ collections::HashMap, io::Read };
use stopwatch::Stopwatch;
use urlencoding;

use base64::{ self, prelude::{ BASE64_STANDARD_NO_PAD, BASE64_URL_SAFE_NO_PAD }, Engine };

use reqwest::{
    blocking::{ get, Client },
    header::{ self, HeaderMap, CONTENT_TYPE, HOST },
    Url,
    Version,
};

use crate::secrets;

//the application needs File.ReadWrite permissions granted for it to work in entra admin center. these can only be granted by an admin.

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
#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hashes {
    pub quick_xor_hash: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub enum FsEntryType {
    File {
        hashes: Hashes,
    },
    #[serde(rename_all = "camelCase")]
    Folder {child_count: usize},
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http)
pub fn get_encoded_sharing_url(client: &Client, url: Url) -> Result<String, Error> {
    let response = client.get(url).send()?;
    let final_url = response.url();
    return Ok(format!("u!{}", BASE64_URL_SAFE_NO_PAD.encode(final_url.as_str())));
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http)
pub fn get_shared_drive_item(
    client: &Client,
    token: &str,
    url: &str
) -> Result<SharedDriveItem, Error> {
    let client = reqwest::blocking::Client::new();
    //let mut params = HashMap::new();

    let share_id = format!("u!{}", BASE64_URL_SAFE_NO_PAD.encode(url.as_bytes()));
    println!("encoded url: {share_id}");
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token).parse()?);
    headers.append(header::CONTENT_TYPE, "application/json".parse()?);
    headers.append("prefer", "redeemSharingLink".parse()?);
    let mut response = client
        .get(format!("{}shares/{}/driveItem", MSAPI_URL, share_id))
        .headers(headers.clone())
        .send()?;
    if response.status().as_u16() != 200 {
        return Err(
            anyhow!("http error - code {}, text: {}", response.status().as_u16(), response.text()?)
        );
    }

    let body = client
        .get(format!("{}shares/{}/driveItem", MSAPI_URL, share_id))
        .headers(headers)
        .send()?
        .text()?;
    println!("response: \n{}", body);

    return Ok(response.json()?);
}

/// [msgraph reference](https://learn.microsoft.com/en-us/graph/api/driveitem-get-content?view=graph-rest-1.0&tabs=http)
pub fn download_item(client: &Client, token: &str, url: &str) -> Result<(), Error> {
    let encodedShareUrl = format!("u!{}", BASE64_URL_SAFE_NO_PAD.encode(url.as_bytes()));
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token).parse()?);

    let response = client
        .get(format!("{}shares/{}/driveItem/content", MSAPI_URL, encodedShareUrl))
        .headers(headers)
        .send()?;

    println!("response code: {}", response.status());
    println!("response : {}", response.bytes()?.len());

    Ok(())
}

///[msgraph reference](https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token)
///TODO: switch to using a certificate
pub fn login(client: &Client) -> Result<String, Error> {
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
        .send()?;

    if response.status().is_success() {
        //println!("access token: {}",response.text().as_ref()?);
        let json: TokenResponse = response.json()?;
        return Ok(json.access_token);
    } else {
        return Err(
            anyhow!("failed to get access token. status code: {}", response.status().as_str())
        );
    }
}
