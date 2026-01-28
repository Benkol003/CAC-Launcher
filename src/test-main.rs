#![allow(unused)]
use std::{error::Error, fs::File};

use log::info;
use reqwest::header::{self, HeaderMap};
use serde_json::to_string_pretty;
use simplelog::WriteLogger;
use src_backend::{ClientCtx, TIMEOUT, configs::{CACDownloadManifest, Config, TMP_DOWNLOADS_FILE}, msgraph::login};

const MSAPI_URL: &str = "https://graph.microsoft.com/v1.0/";

const CAC_SITE_ID: &str = "0jz1q.sharepoint.com,76d3e48c-5ac3-41ef-8655-d6f3af754699,1aacdb15-8ea5-4a63-a2ed-909b8bd53a13";
const CAC_DOC_LIST_ID: &str = "56e4105d-ffac-42e8-8b5f-0edcf87293cd";
const CAC_DOC_DRIVE_ID: &str = "b!jOTTdsNa70GGVdbzr3VGmRXbrBqljmNKou2Qm4vVOhNdEORWrP_oQotfDtz4cpPN";

#[tokio::main]
async fn main() -> Result<(),Box<dyn Error>> {

    //this is to test getting items directly from the msgraph drive
    //rather than creating links for each mod

    WriteLogger::init(simplelog::LevelFilter::Info, simplelog::Config::default(), File::create("CAC-test-main.log").unwrap()).unwrap();

    let ctx = ClientCtx::build()?;
    let token = login(&ctx.client).await?;

    let mut headers = HeaderMap::new();
    headers.append(header::AUTHORIZATION, format!("Bearer {}", token).parse()?);
    headers.append(header::CONTENT_TYPE, "application/json".parse()?);

    
    //let url = format!("{}sites/root/", MSAPI_URL);
    //let url = format!("{}sites/", MSAPI_URL);
    //let url = format!("{}sites/0jz1q.sharepoint.com:/sites/CAC")
    //let url = format!("{}sites/{}/drives", MSAPI_URL,CAC_SITE_ID);
    
    /*
    root
    |_Client
    |_CreamAPI
    |_DLC
    |_Mods
    | |_...
    | |_@${mod_name}[_${version}]?.7z[.${part}]?
    |_Updates
    |_
     */
    let url =  format!("{}/drives/{}/root:/Random/Mods:/children", MSAPI_URL,CAC_DOC_DRIVE_ID);
    let response = ctx.client
        .get(url)
        .headers(headers)
        .timeout(TIMEOUT)
        .send().await?;

    println!("response:");
    let json = response.json::<serde_json::Value>().await?;
    info!("{}",to_string_pretty(&json)?);

    Ok(())
}