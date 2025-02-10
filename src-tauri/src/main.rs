// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Serialize,Deserialize};

fn main() {
    cac_gui_lib::run()
}

#[derive(Clone, Serialize)]
struct Server {
    name: String,
    address: String,
    isOnline: bool,
    players: Vec<String>
}

#[derive(Serialize, Deserialize,Debug)]
struct ServerConfig {
    name: String,
    address: String
}


//TODO default values
#[derive(Serialize, Deserialize,Debug)]
struct Settings {
    username: String,
    servers: Vec<ServerConfig>,
    exilePassword: String,
    modDir: String,
}

/// save new settings to app config submitted from the frontend UI.
#[tauri::command]
fn save_settings(settings: Option<String>) -> bool {
    return false;
}

/// get list of servers as specified in the app config.
#[tauri::command]
fn get_server_list() -> Vec<String> {
    return Vec::new();
}

/// given a server, fetch the server status and detailed player info if available.
#[tauri::command]
fn get_server_status(server: String) -> Vec<String>{
    return Vec::new();
}



/*
    callback list:
get_microsoft_access_token
get_server_list
get_server_players
save_settings
download_or_update_mod
download_dlc
get_server_players (use a2s-rs :) )
*/