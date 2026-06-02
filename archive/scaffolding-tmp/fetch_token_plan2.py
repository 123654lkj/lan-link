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
results = []
for p in [
    "/docs/guides/pricing-token-plan",
    "/docs/token-plan/intro",
    "/docs/token-plan/faq",
]:
    url = "https://platform.minimaxi.com" + p
    status, body = fetch(url)
    if status == 200:
        results.append((p, body))
for p, body in results:
    print("=== " + p + " ===")
    # Filter to interesting content (skip first 100 chars index)
    interesting = body[100:] if body.startswith("> ##") else body
    print(interesting[:4500])
    print()