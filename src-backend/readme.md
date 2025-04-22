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
The 'official' way to download files. WIP

### TODO
what MSVC redists are needed to install with the base game
