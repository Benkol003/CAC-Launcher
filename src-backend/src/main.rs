#![allow(unused_variables)]
#![allow(unreachable_code)]
#![allow(unused_imports)]
#![allow(unused_import_braces)]

#![allow(warnings)]

use std::{env,path::Path};
use anyhow::{anyhow,Error};
use reqwest::Url;

use src_backend::*;

fn main() -> Result<(), Error> {
    //dirhash
    let args: Vec<String> = env::args().collect();
    let p = Path::new(args[1].as_str());
    dirhash::build_dir_manifest(&p,&Path::new("CAC-config/hashes.json"))?;
    Ok(())
}


