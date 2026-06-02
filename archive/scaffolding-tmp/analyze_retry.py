import json, glob, os

files = sorted(glob.glob("/tmp/xingdu_dumps/*_oai_stream.txt"), key=os.path.getmtime)
# v2 deployed at ~12:31 (timestamp 1780288xxx area)
# look at all files

no_tool_count = 0
has_tool_count = 0
total = 0
no_tool_with_intent = 0

# v2 timestamp start ~1780287xxx (12:31)
v2_start = 1780287000

for f in files:
    ts = int(os.path.basename(f).split("_")[0])
    if ts < v2_start:
        continue
    total += 1
    
    text_parts = []
    has_tools = False
    finish = None
    
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
                    fr = ch.get("finish_reason")
                    if fr:
                        finish = fr
                    for tc in delta.get("tool_calls", []):
                        has_tools = True
                    c = delta.get("content", "")
                    if c:
                        text_parts.append(c)
            except:
                pass
    
    text = "".join(text_parts)
    
    if has_tools:
        has_tool_count += 1
    else:
        no_tool_count += 1
        # Check intent keywords
        lower = text.lower()
        has_intent = False
        en_kw = ["let me", "i will", "i should", "i need to", "i have to", "i must", "i'm going to", "i can", "the next step", "i'll ", "now i "]
        zh_kw = ["让我", "我得", "我将", "我需要", "接下来", "现在", "我来", "我要", "先", "开始", "继续"]
        for kw in en_kw:
            if kw in lower:
                has_intent = True
                break
        if not has_intent:
            for kw in zh_kw:
                if kw in text:
                    has_intent = True
                    break
        
        if has_intent:
            no_tool_with_intent += 1
            print("LAZY %s: intent=%s text=%s" % (ts, has_intent, text[:100].replace("\n", " ")))

print("\n=== V2 Stats ===")
print("Total requests: %d" % total)
print("Has tool_calls: %d" % has_tool_count)
print("No tool_calls:  %d" % no_tool_count)
print("No tool + has intent (should have retried): %d" % no_tool_with_intent)
print("No tool + no intent: %d" % (no_tool_count - no_tool_with_intent))