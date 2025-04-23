use core::hash;
use std::{fs::File, path::Path, sync::Mutex};
use anyhow::{anyhow,Error};
use jwalk::WalkDir;
use memmap2::Mmap;
use rayon::prelude::*;
use stopwatch::Stopwatch;
use xxhash_rust::xxh3::{self, xxh3_128, xxh3_64};

pub fn hash_directory(path: &Path) -> Result<u128,Error>{

    let mut hashTable : Mutex<Vec<(String,u128)>> = Mutex::new(Vec::new());

    //par-bridge discards iterator order.
    WalkDir::new(path).sort(false).into_iter().par_bridge()
    .filter(|e| {
        return match e{
            Ok(e2) => e2.file_type().is_file(),
            Err(e3) => panic!("file error: {e3}")
        }
    })
    .for_each(|entry| {
        let path = entry.unwrap().path();
        let path_str: String = path.clone().into_os_string().into_string().unwrap();

        let mut file = File::open(path).unwrap();
    
        let mmap = unsafe { Mmap::map(&file).unwrap() };
        let hash = xxh3_128(&mmap); //TODO benchmark against twox_hash
        let mut lock = hashTable.lock().unwrap();
        lock.push((path_str,hash));
        
    });

    let mut lock = hashTable.lock().unwrap();
    
    //TODO: you need to hash the file and directory names. since you'll do a initial walk anyway so we can do a progress bar that can be done there.

    //sort results so the table order is deterministic / same for same folder contents
    lock.sort_by_key(|i| i.1);
    
    let mut hasher = xxhash_rust::xxh3::Xxh3Default::new();
    for hash in lock.iter().map(|i| i.1){
        hasher.update(&hash.to_le_bytes());
    }
    return Ok(hasher.digest128());
}