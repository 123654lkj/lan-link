import json, glob, os, sys
files = sorted(glob.glob("/tmp/xingdu_dumps/*_oai_stream.txt"), key=os.path.getmtime, reverse=True)[:8]
for f in files:
    ts = os.path.basename(f).split("_")[0]
    tools = []
    text_parts = []
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
                    delta = ch.get("delta", {})
                    for tc in delta.get("tool_calls", []):
                        fn = tc.get("function", {}).get("name", "")
                        if fn and fn not in tools:
                            tools.append(fn)
                    c = delta.get("content", "")
                    if c:
                        text_parts.append(c)
            except:
                pass
    text_preview = "".join(text_parts)[:120].replace("\n", " ")
    print("%s: tools=%s | text=%s" % (ts, tools if tools else "NONE", text_preview if text_preview else ""))