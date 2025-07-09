# CAC-GUI

- `native-tls` for reqwuest is very slow, using `rustls-tls` instead

### Onderive / Sharepoint direct link download
The client first obtains the `FedAuth` cookie from the initial download link. 
This is only valid for the current session and cant be reused for a different download url. You also can't retrieve file info a second time using the same FedAuth cookie.
Then alters the url to the direct download link along with the cookie to download the files.
- [etag](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/ETag) can be used to identify if the
resource at a link changes i.e. to detect a mod version change

- Onedrive links are very slow - about 1000ms response time. This is the fault of the server and not this application. MSGraph API could possibly be faster.

### MSGraph API
The 'official' way to download files.
Each API request takes about 800ms.

### VS Code
- advised to set `"rust-analyzer.cachePriming.enable": false` in settings.json to speed up opening the project

### update handling
The app will redownload the config files on launch and compare for any changes with the existing content manifest, and adds these to the pending updates list in the app config file.
partial downloads are named with id+cTag to avoid resuming a download on a different link.
the app can also use the mod references in the server manifest to know when a server needs updates before launch

### TODO
search for TODO in the src, otherwise:

what MSVC redists are needed to install with the base game
concurrent {download->unzip}'s in the downloader and for ProgressBarBuffer/UI::popup_progress
delete old/existing folder before unzip
check that all mods unzip to folder of their name and isnt nested (or add smth to pull folders up)
try hjson / comments
handle corrupted config files?

Search for arma3_x64.exe
path for arma3_x64.exe wont work if wrapped in quotes
also accept the folder its in?
ask for mod dir
one point of truth for opening and writing config files (maybe lock aswell)
validate after a download that the downloaded folder name matches what the mod is called (or write a tester to download + probe archive contents)
7za.exe progress information in unzip() no longer works