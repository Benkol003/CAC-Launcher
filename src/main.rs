#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{env, fs::{self, File}, io::Write, path::Path};
use anyhow::{anyhow,Error};
use reqwest::Url;

use src_backend::{msgraph::SharedDriveItem, *};

static CONFIG_URL: &str = "https://github.com/Benkol003/CAC-Config/archive/master.zip";

fn main() -> Result<(), Error> {
    //dirhash
    let args: Vec<String> = env::args().collect();
    let p = Path::new(args[1].as_str());
    dirhash::build_dir_manifest(&p,&Path::new("CAC-config/hashes.json"))?;


    { //scope so file is closed before running process
        let mut z7 = File::create("7za.exe")?;
        z7.write_all(Z7_EXE).map_err(|_| anyhow!("failed to unpack 7za.exe"))?;
    }


    fs::remove_file("7za.exe")?;
    Ok(())
}


