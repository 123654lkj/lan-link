import urllib.request, sys
def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0", "Accept": "text/markdown"})
    try:
        resp = urllib.request.urlopen(req, timeout=15)
        body = resp.read().decode("utf-8", errors="replace")
        return resp.status, body
    except Exception as e:
        return None, str(e)
sys.stdout.reconfigure(encoding="utf-8")
# Get the full cache doc
status, body = fetch("https://platform.minimaxi.com/docs/api-reference/anthropic-api-compatible-cache")
if status == 200:
    # skip header line
    body = body[100:]
    print(body)