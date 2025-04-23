use std::collections::HashMap;
use anyhow::{anyhow,Error};
use stopwatch::Stopwatch;
use urlencoding;

use serde::Deserialize;
use base64::{self, prelude::{BASE64_STANDARD_NO_PAD, BASE64_URL_SAFE_NO_PAD},Engine};

use reqwest::{blocking::{get,Client}, header::{self, HeaderMap, CONTENT_TYPE, HOST}, Version};

use crate::secrets;

//the application needs File.ReadWrite permissions granted for it to work in entra admin center. these can only be granted by an admin.

const TENANT_ID: &str = "4fd01353-8fd7-4a18-a3a1-7cd70f528afa";
const APP_CLIENT_ID: &str = "9ecaa0e8-9caf-4f49-94e8-8430bbf57486";
//const MSGPRAPH_KEY - place in secrets.rs

//see https://learn.microsoft.com/en-us/graph/api/shares-get?view=graph-rest-1.0&tabs=http

#[derive(Deserialize)]
struct TokenResponse{
    token_type : String,
    expires_in: usize,
    ext_expires_in: usize,
    access_token: String
}

#[derive(Deserialize,Debug)]
struct sharedDriveItem {
    id: String,
    name: String,
    owner: String,
}

pub fn getSharedDriveItem(client: &Client, token: &str, url: &str) -> Result<(),Error> {
let client = reqwest::blocking::Client::new();
//let mut params = HashMap::new();

let encodedShareUrl = format!("u!{}",BASE64_URL_SAFE_NO_PAD.encode(url.as_bytes()));
println!("encoded url: {encodedShareUrl}");
    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}",token).parse()?);
    headers.append(header::CONTENT_TYPE,"application/json".parse()?);
    headers.append("prefer","redeemSharingLink".parse()?);

    let msapi_url = "https://graph.microsoft.com/v1.0/";
    let mut clock = Stopwatch::start_new();
    let response = client.get(format!("{}shares/{}/driveItem",msapi_url,encodedShareUrl)).headers(headers).send()?;
    clock.stop();
    println!("getSharedDriveItem response time: {}ms",clock.elapsed_ms());
    if(response.status().as_u16() != 200) {
        return Err(anyhow!("http error - code {}, text: {}",response.status().as_u16(),response.text()?));
    }

    let sharedDriveItem: String = response.text()?;
    //println!("sharedDriveItem:\n{sharedDriveItem}");

    return Ok(());
}

//not valid for app token / us TODO REMOVE
pub fn login_info(client: &Client, token: &str) -> Result<(),Error>{
    let mut headers = HeaderMap::new();
    headers.insert(header::AUTHORIZATION, format!("Bearer {}",token).parse()?);
    headers.insert(header::HOST,"graph.microsoft.com".parse()?);

    let response = client.get("https://graph.microsoft.com/v1.0/me/").version(Version::HTTP_11).headers(headers).send()?;

    if(!response.status().is_success()){
        return Err(anyhow!("failed to get login info - http error code {}, text: {}",response.status().as_u16(),response.text()?));
    }

    let info = response.text()?;
    println!("login info:\n{}",info);

    return Ok(());   
}

//TODO: switch to using a certificate 
pub fn login(client: &Client) -> Result<String,Error> {
//get an access token
let mut params = HashMap::new();
params.insert("client_id",APP_CLIENT_ID);
params.insert("scope","https://graph.microsoft.com/.default");
params.insert("client_secret",secrets::MSGRAPH_KEY);
params.insert("grant_type","client_credentials");

let response = client.post(format!("https://login.microsoftonline.com/{TENANT_ID}/oauth2/v2.0/token"))
.header(HOST, "login.microsoftonline.com")
.header(CONTENT_TYPE,"application/x-www-form-urlencoded")
.form(&params)
.send()?;

if response.status().is_success() {
    //println!("access token: {}",response.text().as_ref()?);
    let json: TokenResponse = response.json()?;
    return Ok(json.access_token);
}else{
    return Err(anyhow!("failed to get access token. status code: {}",response.status().as_str()));
};      
}