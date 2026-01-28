use std::{collections::HashMap, fs::DirEntry, path::{self, PathBuf}, time::Duration};

use anyhow::{anyhow};
use serde::Deserialize;
use a2s::{A2SClient,info::Info};
use tokio::task::JoinHandle;
use crate::{configs::{Config, *}, *};

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
        let config = CACConfig::read()?;
        let mod_dir = config.absolute_mod_dir()?;
        let mut args = LAUNCH_ARGS.clone();
        args.push(format!("-connect={}",self.address));
        args.push(format!("-port={}",self.port));
        args.push(format!(r#"-name="{}""#,config.username));

        let mut mod_arg: String = r#"-mod="#.into();
        let opt_mod_iter: Box<dyn Iterator<Item = &String>> = if config.optionals_on {
            Box::new(std::iter::empty::<&String>().into_iter())
        }else {
            Box::new(config.enabled_optionals.iter())
        };
        self.mods.iter().chain(config.enabled_optionals.iter()).chain(opt_mod_iter).for_each(|x|{
            mod_arg+="\""; 
            if(x.chars().nth(0).unwrap()=='@'){
                mod_arg+=mod_dir.join(x).as_os_str().to_str().unwrap();
            }else{ //is dlc
                mod_arg+=x; 
            }
            mod_arg+="\";";
        });
        args.push(mod_arg);
        if self.password {
            args.push(format!(r#"-password="{}""#,config.server_password));
        }
        let args_expanded  =args.iter().fold(String::new(),|i,x|{i+" "+x});
        log::warn!("launching arma 3 with args (len {}): '{}'",args_expanded.len(),args_expanded); //TODO RM 
        std::process::Command::new(config.arma_path).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        Ok(())
    }
}

pub fn read_config() -> Result<Vec<(String,Server)>, Error> {
    let conf_path = SERVERS_FILE.to_path_buf();
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


pub fn update_list() -> Result<Vec<(String,Vec<String>)>,Error> {
    let config = CACConfig::read()?;

    let moddir = std::fs::read_dir(config.absolute_mod_dir()?)?;
    let arma_pb = PathBuf::from_str(&config.arma_path)?;
    let arma_dir_pb = arma_pb.parent().unwrap().to_str().unwrap();
    
    let arma_dir = std::fs::read_dir(arma_dir_pb)?;

    warn!("arma dir {}",arma_dir_pb);

    let cme = |e| {
             warn!("error iterating mod directory: {}",e); None
    };
    let dir_filter = |x: Result<DirEntry, std::io::Error>|{
        x.map_or_else(cme,|x|{
            x.file_type().map_or_else(cme, |t| {
                match t.is_dir() {
                    true => {Some(x.file_name().into_string().unwrap())}
                    false => { None }
                }
            })
        })
    };
    let arma_folders: Vec<_> = arma_dir.filter_map(dir_filter).collect(); //for dlc, 
    let mods_present: Vec<_> = moddir.filter_map(dir_filter).collect();

    let servers = read_config()?;
    let ret: Vec<(String,Vec<String>)> = servers.iter().map(|(name,server)| {
        let update_list = server.mods.iter().filter_map(|x| {

            //assuming 'mods' without @ to be dlc
            match if x.starts_with("@") {mods_present.contains(x)} else {arma_folders.contains(x)} {
                true => {None}
                false => {
                    Some(x.clone())
                }
            }
        }).chain(
            server.mods.iter().filter_map(|x|{
                match config.pending_updates.contains(x) {
                    true => {
                        Some(x.clone())
                    }
                    false => {None}
                }
            })
        ).collect::<Vec<_>>();
        (name.clone(),update_list)
    }).collect();
    Ok(ret)
    
}