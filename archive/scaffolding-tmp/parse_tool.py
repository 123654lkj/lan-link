import json
f = "/tmp/xingdu_dumps/1780288860_oai_stream.txt"
with open(f) as fh:
    for line in fh:
        raw = line.strip()
        if raw.startswith("data: "):
            raw = raw[6:]
        if raw == "[DONE]":
            continue
        try:
            d = json.loads(raw)
            for ch in d.get("choices", []):
                for tc in ch.get("delta", {}).get("tool_calls", []):
                    fn = tc.get("function", {})
                    args = fn.get("arguments", "")
                    if fn.get("name"):
                        print("tool: %s  args: %s" % (fn["name"], args[:300]))
        except:
            pass