import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

// Fetch the Microsoft access token from the backend
const getAccessToken = async () => {
  try {
    const response = await invoke("get_microsoft_access_token");
    return response;
  } catch (err) {
    console.error("Error getting access token:", err);
    return null;
  }
};

function App() {
  const [serverList, setServerList] = useState([]);
  const [selectedServer, setSelectedServer] = useState(null);
  const [playerList, setPlayerList] = useState([]);
  const [error, setError] = useState(null);
  const [settingsVisible, setSettingsVisible] = useState(false);
  const [username, setUsername] = useState("");
  const [optionalMods, setOptionalMods] = useState([]);
  const [modsDirectory, setModsDirectory] = useState("");
  const [dlcs, setDlcs] = useState([]);
  const [isLoading, setIsLoading] = useState(true);

  // Fetch server list on component mount
  useEffect(() => {
    const fetchServerList = async () => {
      try {
        const data = await invoke("get_server_list");
        setServerList(data);
      } catch (err) {
        setError("Error fetching server list.");
        console.error(err);
      } finally {
        setIsLoading(false);
      }
    };

    fetchServerList();
  }, []);

  // Fetch players for the selected server
  const fetchPlayers = async (ip, port) => {
    try {
      const players = await invoke("get_server_players", { ip, port });
      setPlayerList(players);
    } catch (err) {
      setError("Error fetching players.");
      console.error(err);
    }
  };

  // Handle server selection
  const handleServerClick = (server) => {
    setSelectedServer(server);
    fetchPlayers(server.ip, server.port);
  };

  // Toggle settings popup visibility
  const handleSettingsToggle = () => {
    setSettingsVisible((prevState) => !prevState);
  };

  // Save user settings
  const handleSaveSettings = async () => {
    try {
      await invoke("save_settings", { username, optionalMods, modsDirectory, dlcs });
      console.log("Settings saved successfully.");
    } catch (err) {
      console.error("Failed to save settings:", err);
    }
  };

  // Handle mod action (download/update)
  const handleModAction = async (modName) => {
    try {
      await invoke("download_or_update_mod", { modName });
      console.log(`Mod action completed for: ${modName}`);
    } catch (err) {
      console.error(`Failed to update/download mod: ${modName}`, err);
    }
  };

  // Handle DLC download
  const handleDlcDownload = async (downloadLink) => {
    try {
      const token = await getAccessToken();
      if (!token) {
        setError("Authentication failed. Please log in again.");
        return;
      }
      await invoke("download_dlc", { downloadLink, token });
      console.log("DLC download initiated.");
    } catch (err) {
      console.error("Error downloading DLC:", err);
      setError("Error downloading DLC.");
    }
  };

  return (
    <main className="container">
      <div className="div1">
        <img src="/logo-transparent-svg.svg" className="logo react" alt="Logo" />
        <h1>Server List</h1>
        <div className="servers">
          {error && <p className="error">{error}</p>}
          {isLoading ? (
            <p>Loading servers...</p>
          ) : serverList.length > 0 ? (
            <ul>
              {serverList.map((server, index) => (
                <li
                  key={index}
                  onClick={() => handleServerClick(server)}
                  className={selectedServer === server ? "active" : ""}
                >
                  <strong>{server.name}</strong>
                  <p>IP: {server.ip}</p>
                  <p>Status: Active</p>
                </li>
              ))}
            </ul>
          ) : (
            <p>No servers available.</p>
          )}
        </div>
      </div>

      <div className="div2">
        {selectedServer && !isLoading && (
          <div className="server-details">
            <h2>{selectedServer.name} - Players</h2>
            <ul className="player-list">
              {playerList.length > 0 ? (
                playerList.map((player, index) => (
                  <li key={index}>
                    <strong>{player.name}</strong>
                  </li>
                ))
              ) : (
                <p>Loading players...</p>
              )}
            </ul>

            <h3>Required Mods</h3>
            <ul className="mod-list">
              {selectedServer.requiredMods?.length > 0 ? (
                selectedServer.requiredMods.map((mod, index) => (
                  <li key={index} className="mod-item">
                    <div className="mod-info">
                      <strong>{mod.name}</strong>
                      <p>Last Updated: {mod.lastUpdated}</p>
                    </div>
                    <button className="mod-action-btn" onClick={() => handleModAction(mod.name)}>
                      {mod.isInstalled ? "Update" : "Download"}
                    </button>
                  </li>
                ))
              ) : (
                <p>No required mods listed.</p>
              )}
            </ul>

            <h3>DLCs</h3>
            <ul className="dlc-list">
              {dlcs.length > 0 ? (
                dlcs.map((dlc, index) => (
                  <li key={index} className="dlc-item">
                    <div className="dlc-info">
                      <strong>{dlc.name}</strong>
                      <button className="dlc-action-btn" onClick={() => handleDlcDownload(dlc.downloadLink)}>
                        {dlc.isInstalled ? "Update" : "Download"}
                      </button>
                    </div>
                  </li>
                ))
              ) : (
                <p>No DLCs available.</p>
              )}
            </ul>
          </div>
        )}

        {settingsVisible && (
          <div className="settings-popup">
            <h3>Settings</h3>
            <button className="close-btn" onClick={handleSettingsToggle}>&times;</button>
            <label>
              Arma 3 Username:
              <input type="text" value={username} onChange={(e) => setUsername(e.target.value)} />
            </label>

            <div>
              <h4>Optional Mods</h4>
              {selectedServer?.optionalMods?.map((mod, index) => (
                <label key={index}>
                  <input
                    type="checkbox"
                    checked={optionalMods.includes(mod)}
                    onChange={() => setOptionalMods((prev) => prev.includes(mod) ? prev.filter((item) => item !== mod) : [...prev, mod])}
                  />
                  {mod}
                </label>
              ))}
            </div>

            <div>
              <label>
                Mods Directory:
                <input type="text" value={modsDirectory} onChange={(e) => setModsDirectory(e.target.value)} />
              </label>
            </div>

            <div>
              <h4>DLCs</h4>
              <input
                type="text"
                value={dlcs.join(",")}
                onChange={(e) => setDlcs(e.target.value.split(","))}
                placeholder="Enter CDLCs and DLCs (comma separated)"
              />
            </div>

            <button onClick={handleSaveSettings}>Save</button>
          </div>
        )}

        <button className="settings-btn" onClick={handleSettingsToggle}>
          ⚙️ Settings
        </button>
      </div>
    </main>
  );
}

export default App;
