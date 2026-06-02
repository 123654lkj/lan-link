import json, glob, os
files = sorted(glob.glob("/tmp/xingdu_dumps/*_oai_stream.txt"), key=os.path.getmtime, reverse=True)[:5]
for f in files:
    ts = os.path.basename(f).split("_")[0]
    tool_calls = {}
    cur_name = None
    cur_args = ""
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
                        if fn.get("name"):
                            if cur_name and cur_args:
                                tool_calls[cur_name] = cur_args[:200]
                            cur_name = fn["name"]
                            cur_args = fn.get("arguments", "")
                        else:
                            cur_args += fn.get("arguments", "")
            except:
                pass
    if cur_name and cur_args:
        tool_calls[cur_name] = cur_args[:200]
    for name, args in tool_calls.items():
        print("%s | %s | %s" % (ts, name, args.replace("\n", " ")[:150]))
    if not tool_calls:
        print("%s | (no tools)" % ts)