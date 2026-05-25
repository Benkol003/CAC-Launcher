```
Usage: downloader.exe [OPTIONS] <-f <FILE_URL_LIST>|URL>

Arguments:
  [URL]...

Options:
  -o <OUTPUT_DIR>     [default: "."]
  -f <FILE_URL_LIST>
  -h, --help          Print help
  -V, --version       Print version
```

examples links_file.txt

```
https://tinyurl.com/567kx3mc
https://tinyurl.com/2j6kzpyn
```
example usage: `./downloader.exe -f links.txt -o ./Mods/`