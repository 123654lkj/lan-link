import urllib.request, re
url = "https://platform.minimaxi.com/docs/api-reference/anthropic-api-compatible-cache"
req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0", "Accept": "text/markdown"})
try:
    resp = urllib.request.urlopen(req, timeout=15)
    body = resp.read().decode("utf-8", errors="replace")
    print("status:", resp.status)
    print("content-type:", resp.headers.get("content-type"))
    print("len:", len(body))
    print(body[:3000])
except Exception as e:
    print("err:", e)