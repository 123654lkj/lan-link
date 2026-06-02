import urllib.request
def fetch(url):
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0", "Accept": "text/markdown"})
    try:
        resp = urllib.request.urlopen(req, timeout=15)
        body = resp.read().decode("utf-8", errors="replace")
        return resp.status, body
    except Exception as e:
        return None, str(e)
for p in [
    "/docs/guides/pricing-token-plan",
    "/docs/guides/pricing-token-plan-team",
    "/docs/token-plan/intro",
    "/docs/token-plan/faq",
    "/docs/token-plan/minimax-cli",
]:
    url = "https://platform.minimaxi.com" + p
    status, body = fetch(url)
    if status == 200:
        # Decode GBK if needed
        try:
            body.encode("utf-8").decode("utf-8")
        except:
            try:
                body = body.encode("latin-1").decode("utf-8")
            except:
                pass
        print("=== " + p + " ===")
        print(body[:3500])
        print()
    else:
        print("miss:", p, status)