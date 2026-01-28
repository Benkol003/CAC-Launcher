use std::{collections::{HashMap, HashSet}, fs::{File, OpenOptions}, io::Read, path::{self, PathBuf}};
use crate::{UI::TUI};
use anyhow::{anyhow,Error};
use chrono::format::StrftimeItems;
use log::warn;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub const CONFIG_FOLDER: Lazy<PathBuf> = Lazy::new(|| {
    PathBuf::from("CAC-Config")
});
pub const TMP_FOLDER: Lazy<PathBuf> = Lazy::new(|| {
    CONFIG_FOLDER.join("tmp")
});

pub const SERVERS_FILE: Lazy<PathBuf> = Lazy::new(|| {
    CONFIG_FOLDER.join("servers.json")
});
pub const CONFIG_FILE: Lazy<PathBuf> = Lazy::new(|| {
    CONFIG_FOLDER.join("config.json")
});

pub const CONTENT_FILE: Lazy<PathBuf> = Lazy::new(|| {
    CONFIG_FOLDER.join("content.json")
});

/// associated ID's for downloaded temp files.
/// 7zip needs parts to be named @name.7z.00x
pub const TMP_DOWNLOADS_FILE: Lazy<PathBuf> = Lazy::new(|| {
    CONFIG_FOLDER.join("tmp-downloads.json")
});

pub trait Config: Serialize + for<'de> Deserialize<'de> {
    fn file_path() -> PathBuf;
    fn save(&self) -> Result<(),Error> {
        let f = OpenOptions::new().truncate(true).write(true).create(true).open(Self::file_path())?;
        serde_json::to_writer_pretty(f, &self)?;
        Ok(())
    }

    fn _read(path: PathBuf) -> Result<Self,Error> {
        let mut config_buf = String::new();
        let mut config_file = File::open(path)?;
        config_file.read_to_string(&mut config_buf)?;
        Ok(serde_json::from_str::<Self>(config_buf.as_str())?)
    }

    fn read() -> Result<Self,Error> {
        Self::_read(Self::file_path())
    }
} //TODO save on drop - set a unsaved bool for changes (either wrap all mut fn's or just flag if get mut ref), panic in drop

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct TmpDownloadID {
    pub id: String,
    pub etag: String
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CACDownloadManifest(pub HashMap<String,TmpDownloadID>);


impl Config for CACDownloadManifest {
    fn file_path() -> PathBuf {
         TMP_DOWNLOADS_FILE.to_path_buf()
    }
}

//TODO: remove mods from pending updates if they dont exist in content.json anymore (if client missed update and then it was removed from the server)

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CACConfig {
    pub username: String,
    pub arma_path: String,
    //shared between all servers that need it. TODO add to servers.json if a server requires a password (Option<bool> with default?)
    pub server_password: String,
    pub optionals_on: bool,
    pub enabled_optionals: HashSet<String>,
    pub pending_updates: HashSet<String>,
    mod_dir: String //access via absolute_mod_dir instead 
}

impl Config for CACConfig {
    fn file_path() -> PathBuf {
        CONFIG_FILE.to_path_buf()
    }
}

impl CACConfig {
    pub fn absolute_mod_dir(&self) -> Result<PathBuf,Error> {
        //arma will crash if moddir contains relative e.g. "./" ("Mods/ is fine"), so resolve if is the case
        //dont store the absolute path though, then can move folders around without stuff breaking
        
        //windows paths only
        let starts_drive: bool = !cfg!(windows) || {
            let (b,e) = self.mod_dir.split_at(0);
            e.starts_with(":/") || e.starts_with(";\\")
        };

        if self.mod_dir.starts_with("./") || 
        self.mod_dir.starts_with(".\\") || 
        self.mod_dir.starts_with("../") || 
        self.mod_dir.starts_with(".\\") || 
        self.mod_dir.starts_with("\\") ||
        self.mod_dir.starts_with("/") ||
        starts_drive {
            Ok(path::absolute(PathBuf::from(&self.mod_dir).parent().unwrap())
            .map_err(|_| anyhow!("failed to get absolute path of config.mod_dir"))?.into())
        }else {
            //arma folder is at parent of ...exe
            Ok(PathBuf::from(&self.arma_path).parent().unwrap().join(&self.mod_dir))
        }
    }
}


impl CACConfig {
    // fn import_caccore() -> Self {
    //     //TODO: need to find arma directory first / get from user ./CACCore
    // }

    pub fn default(ui: &mut TUI) -> Result<Self,Error> {
        //find arma
        let mut ap = "./arma3_x64.exe".to_string();
        match std::fs::metadata(&ap) {
            Ok(md) => {
                if(!md.is_file()){
                    return Err(anyhow!("found arma at './arma3_x64.exe' but it is not a file")); //TODO not a fatal error?
                } 
            }
            
            //TODO enter folder instead not path to exe
            Err(_) => {
                loop {
                    ap=ui.popup_text_entry("Enter path to your arma folder or 'arma3_x64.exe'").ok_or(anyhow!("path to arma 3 executable not provided"))?
                    .replace("\"", ""); //trim double quotes if using 'copy from path' option in windows
                    match std::fs::metadata(&ap) {
                        Ok(md) => {
                            warn!("arma 3 ap: {}",ap.as_str());
                            if(md.is_dir()){
                                warn!("arma 3 ap is dir");
                                let ap_bin_pb = PathBuf::from(&ap).join("arma3_x64.exe");
                                let ap_bin_md = std::fs::metadata(&ap_bin_pb)?;
                                if(!ap_bin_md.is_file()){
                                     ui.popup_blocking_prompt(format!("'{}' is not a file, try again.",&ap_bin_pb.display()).into());
                                     continue;
                                }
                                let ap_bin = ap_bin_pb.as_os_str().to_str().ok_or(anyhow!("failed to convert PathBuf to str"))?;
                                ap = ap_bin.to_string();
                                break;
                            }
                            if(!md.is_file()){
                                ui.popup_blocking_prompt(format!("'{}' is not a file, try again.",ap).into());
                                continue;
                            }
                            else{
                                break;
                            }
                        }
                        Err(_) => {
                            ui.popup_blocking_prompt(format!("'{}' does not exist, try again.",ap).into());
                            continue;
                        }   
                    }
                }
            }
        }

        //TODO: prompt for abs/rel mod directory with default rel "Mods/"

        Ok(CACConfig {
            arma_path: ap.clone(),
            username: whoami::username(),
            server_password: String::new(),
            enabled_optionals: HashSet::new(),
            optionals_on: false,
            pending_updates: HashSet::new(),
            mod_dir: PathBuf::from(ap).parent().unwrap().join("Mods").to_str().unwrap().into()
        })
    }
}


#[derive(Serialize, Deserialize,Debug,Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum Links {
    Single(String),
    Multilink(Vec<String>),
}

pub struct LinksIter<'a> {
    inner: std::slice::Iter<'a, String>,
}

impl<'a> IntoIterator for &'a Links {
    type Item = &'a String;
    type IntoIter = LinksIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Links::Single(s) => LinksIter {
                inner: std::slice::from_ref(s).iter()
            },
            Links::Multilink(vec) => LinksIter {
                inner: vec.iter(),
            },
        }
    }
}

impl<'a> Iterator for LinksIter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

#[derive(Serialize, Deserialize,Debug,Clone)]
pub struct DLC {
    link: Links,
    pwd: String,
    description: String
}

#[derive(Serialize, Deserialize, Debug, Clone,Default)]
#[serde(rename_all = "camelCase")]
pub struct CACContent {
    pub mods : HashMap<String,Links>,
    pub optionals: HashMap<String,Links>,
    //TODO: arma base game
    pub dlc: HashMap<String,DLC>,
}

impl Config for CACContent {
    fn file_path() -> PathBuf {
        CONTENT_FILE.to_path_buf()
    }
}

impl CACContent {

    pub fn read_from(path: PathBuf) -> Result<Self,Error> {
        Self::_read(path)
    }

    /// # Returns: combined iterator over all content items in the manifest. 
    pub fn content_iter<'a>(&'a self) -> impl Iterator<Item = (&'a String,&'a Links)> {
        self.dlc.iter().map(|x| (x.0,&x.1.link)).chain(self.mods.iter().chain(self.optionals.iter())).into_iter()
    }

    /// # Returns: combined hashmap of all content items in the manifest.
    /// TODO: return error if try to exist existing key?
    pub fn content_map<'a>(&'a self) -> HashMap::<&'a String, &'a Links> {
        let mut ret = HashMap::<&'a String, &'a Links>::new();
        ret.extend(self.dlc.iter().map(|x| (x.0,&x.1.link)));
        ret.extend(self.mods.iter());
        ret.extend(self.optionals.iter());
        ret
    }
    
}