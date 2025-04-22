import requests, mimetypes
from tqdm import tqdm
import time
file_url = "https://tinyurl.com/5c6cnx96"
save_path = 'sharepoint_downloaded'

def sizeof_fmt(num, suffix="B"):
    for unit in ("", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi"):
        if abs(num) < 1024.0:
            return f"{num:3.1f}{unit}{suffix}"
        num /= 1024.0
    return f"{num:.1f}Yi{suffix}"

# Make GET request with allow_redirect
start = time.time()
res = requests.get(file_url, allow_redirects=True)
end = time.time()
print(f"request time: {end-start}")
if res.status_code == 200:
    # Get redirect url & cookies for using in next request
    new_url = res.url
    cookies = res.cookies.get_dict()
    
    # Do some magic on redirect url
    new_url = new_url.replace("onedrive.aspx","download.aspx").replace("?id=","?SourceUrl=")

    print(f"new download url: \n{new_url}")

    # Make new redirect request
    headers = {'Range': 'bytes=0-4096'}
    with requests.get(new_url, cookies=cookies, stream=True,headers=headers) as response:
        if response.status_code == 200 or response.status_code == 206:
            size = int(response.headers.get('Content-Length', 0))
            content_disposition = response.headers.get('Content-Disposition')

            if content_disposition:
                filename = content_disposition.split('filename="')[1].split('"')[0]
            else:
                filename = new_url.split('/')[-1].split('?')[0]

            save_path = filename

            chunk_size = 65536  # 64kb
            downloaded = 0
            print(f"file size: {sizeof_fmt(size)}")
            progress_bar = tqdm(total=int(size / chunk_size), desc="downloading file...")

            with open(save_path, "wb") as file:
                for chunk in response.iter_content(chunk_size=chunk_size):
                    progress_bar.update(1)
                    file.write(chunk)
                    downloaded += len(chunk)

            progress_bar.close()
            print(f"file downloaded as {save_path}!")
        else:
            print(f"HTTP error downloading file: {response.status_code}")