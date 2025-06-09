use std::{collections::HashMap, path::PathBuf, time::Duration};

use anyhow::{anyhow, Error };
use serde::Deserialize;
use a2s::{A2SClient,info::Info};
use tokio::task::JoinHandle;
use crate::*;

#[derive(Deserialize,Debug)]
pub struct Server {
    pub address: String,
    pub port: u16,
    pub mods: Vec<String>,
}

pub fn read_config() -> Result<Vec<(String,Server)>, Error> {
    let conf_path = PathBuf::from(CONFIG_FOLDER).join("servers.json");
    if !std::fs::exists(&conf_path)? {
        return Err(anyhow!("servers.json config file not found"));
    }
    let mut file = File::open(&conf_path)?;
    let mut content = String::new();
    file.read_to_string(&mut content);
    Ok(serde_json::from_str::<HashMap<String,Server>>(&content)?.into_iter().collect())
}

/// if we fail to get info about a server, we assume its offline and return None.
pub async fn status(servers: &Vec<(String,Server)>) -> Result<Vec<(String,Option<Info>)>,Error> {
    
    //spawn tasks and collect them so they spawn in parallel.
    let tasks: HashMap<_,_> = servers.iter().map(|(k,v)| {
        let connect = format!("{}:{}",v.address,v.port+1);//steam query port is +1
        let k = k.clone();
        (k,tokio::spawn(async move {
            let mut client = A2SClient::new().await?;
            client.set_timeout(Duration::from_millis(500));

            //any error retreiving server info we convert to None
            Ok::<Option<Info>,Error>(client.info(connect).await.map_or(None, |i| Some(i)))
        }))
    }).collect();

    //now join all tasks or propogate first error we come across
    let mut ret = Vec::new(); 
    for (k,v) in tasks {
        ret.push((k, v.await??));
    }
    Ok(ret)
}   