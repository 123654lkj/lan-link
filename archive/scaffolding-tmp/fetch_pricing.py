import urllib.request
def fetch(path):
    url = "https://platform.minimaxi.com" + path
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0", "Accept": "text/markdown"})
    try:
        resp = urllib.request.urlopen(req, timeout=15)
        body = resp.read().decode("utf-8", errors="replace")
        return resp.status, body
    except Exception as e:
        return None, str(e)
for p in ["/docs/pricing/overview", "/docs/pricing/coding-plan", "/docs/pricing/token-plan", "/docs/pricing/plan", "/docs/pricing/coding-plan-overview"]:
    status, body = fetch(p)
    if status == 200 and "Anthropic" not in body[:50]:
        print("FOUND:", p, len(body))
        print(body[:2500])
        print("---END---")
        break
    else:
        print("miss:", p, "status:", status)