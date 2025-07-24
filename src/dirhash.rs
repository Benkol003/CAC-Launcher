use anyhow::{anyhow, Error};
use size::Size;
use core::hash;
use jwalk::WalkDir;
use memmap2::Mmap;
use rayon::prelude::*;
use serde::Serialize;
use std::io::{Read, Write};
use std::{
    fs::{self, File},
    hash::Hasher,
    path::Path,
    sync::Mutex,
};
use stopwatch::Stopwatch;
use xxhash_rust::xxh3::{self, xxh3_128, xxh3_64, Xxh3};

//TODO: progress bar
//TODO: symlink

pub fn build_dir_manifest(base: &Path, manifestPath: &Path) -> Result<(), Error> {

    if(!fs::exists(base)?) {
        return Err(anyhow!("path '{}' does not exist",{base.as_os_str().to_str().ok_or(anyhow!("failed to convert &OsStr to &Str"))?})); 
    }

    let mut clock = Stopwatch::start_new();

    let mut manifest = serde_json::Map::new();

    let mut entries = WalkDir::new(base).min_depth(1).max_depth(1).into_iter();

    while let Some(e) = entries.next() {
        let entry = e?;
        if (entry.file_type().is_dir()) {
            println!(
                "hashing {}",
                entry
                    .path()
                    .as_os_str()
                    .to_str()
                    .ok_or(anyhow!("failed to convert &OsStr to &Str"))?
            ); //this is excessive...
            let dpbuf = entry.path();
            let dp = dpbuf.as_path();
            manifest.insert(
                dp.strip_prefix(base)?
                    .to_path_buf()
                    .into_os_string()
                    .into_string()
                    .map_err(|_| anyhow!("failed to convert OsString to String"))?,
                hash_directory(dp)?.to_string().into(),
            ); //json only supports 64bit ints
        }
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(manifestPath)?;
    let mut writer = std::io::BufWriter::new(file);

    let mv = serde_json::Value::Object(manifest);
    serde_json::to_writer_pretty(writer, &mv)?;

    clock.stop();
    println!("manifest built in {}s", clock.elapsed().as_secs());

    return Ok(());
}

pub fn hash_directory(base_path: &Path) -> Result<u128, Error> {
    let mut hashTable: Mutex<Vec<u128>> = Mutex::new(Vec::new());
    let _ = WalkDir::new(base_path)
        .sort(false)//par-bridge discards iterator order.
        .into_iter()
        .par_bridge()
        .try_for_each(|e| -> Result<(), Error> {
            let entry = e?;
            let path = entry.path();
            let rel_path = path.strip_prefix(base_path)?;

            let mut hasher = Xxh3::new();
            hasher.update(rel_path.as_os_str().as_encoded_bytes());

            let ftype = entry.file_type();
            if ftype.is_file() {

                //TODO RM
                let fsize = Size::from_bytes(entry.metadata()?.len());
                println!("{}, size: {}",path.as_os_str().to_str().unwrap(),fsize.format()); 

                hasher.write_u8(0);

                //hash file contents
                let mut file = File::open(path)?;
                let mmap = unsafe { Mmap::map(&file)? };
                hasher.update(&mmap);
            } else if ftype.is_dir() {
                hasher.write_u8(1);
            } else if ftype.is_symlink() {
                hasher.write_u8(2);
                let dest = fs::read_link(path)?;
                hasher.update(dest.as_os_str().as_encoded_bytes());
            } else {
                //skip
            }

            let mut lock = hashTable.lock().unwrap();
            lock.push(hasher.digest128());
            return Ok(());
        });

    let mut lock = hashTable.lock().unwrap();

    //sort results so the table order is deterministic / same order for same folder contents.
    lock.sort();

    let mut hasher = xxhash_rust::xxh3::Xxh3Default::new();
    for hash in lock.iter() {
        hasher.update(&hash.to_le_bytes());
    }
    return Ok(hasher.digest128());
}
