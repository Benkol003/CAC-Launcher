use std::{collections::HashMap, path::{self, PathBuf}, time::Duration};

use anyhow::{anyhow, Error };
use serde::Deserialize;
use a2s::{A2SClient,info::Info};
use tokio::task::JoinHandle;
use crate::{configs::CACConfig, *};

#[derive(Deserialize,Debug)]
pub struct Server {
    pub address: String,
    pub port: u16,
    pub mods: Vec<String>,

    #[serde(default)]
    pub password: bool
}

const LAUNCH_ARGS: Lazy<Vec<String>> = Lazy::new(|| {
    vec!["-noSplash", "-skipIntro", "-hugePages", "-setThreadCharacteristics", "-EnableHT"].iter().map(|x| x.to_string()).collect()
});


impl Server {
    pub fn launch(&self) -> Result<(),Error> {
        let mut config_buf = String::new();
        let mut config_file = File::open(CONFIG_FILE.as_path())?;
        config_file.read_to_string(&mut config_buf)?;
        let mut config: CACConfig = serde_json::from_str::<CACConfig>(&config_buf)?;
        let mod_dir = config.absolute_mod_dir()?;
        let mut args = LAUNCH_ARGS.clone();
        args.push(format!("-connect={}",self.address));
        args.push(format!("-port={}",self.port));
        args.push(format!(r#"-name="{}""#,config.username));

        let mut mod_arg: String = r#""-mod="#.into();
        let opt_mod_iter: Box<dyn Iterator<Item = &String>> = if config.optionals_on {
            Box::new(std::iter::empty::<&String>().into_iter())
        }else {
            Box::new(config.enabled_optionals.iter())
        };
        self.mods.iter().chain(config.enabled_optionals.iter()).chain(opt_mod_iter).for_each(|x|{ 
            if(x.chars().nth(0).unwrap()=='@'){
                mod_arg+=mod_dir.join(x).as_os_str().to_str().unwrap();
            }else{ //is dlc
                mod_arg+=x; 
            }
            mod_arg+=";";
        });
        mod_arg+=r#"""#;
        args.push(mod_arg);
        if self.password {
            args.push(format!(r#"-password="{}"#,config.server_password));
        }
        log::warn!("launching arma 3 with args: '{}'",args.iter().fold(String::new(),|i,x|{i+" "+x})); //TODO RM 
        std::process::Command::new(config.arma_path).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        Ok(())
    }
}

pub fn read_config() -> Result<Vec<(String,Server)>, Error> {
    let conf_path = CONFIG_FOLDER.join("servers.json");
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