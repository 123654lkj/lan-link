import re
with open(r"G:\codex-AI-tools\tmp\minimax_cache_doc.html", "rb") as f:
    raw = f.read()
# Try utf-8 first, fall back
try:
    content = raw.decode("utf-8")
except:
    content = raw.decode("gbk", errors="replace")
chunks = re.findall(r"self\.__next_f\.push\(\[1,\"(.*?)\"\]", content, re.DOTALL)
all_text = ""
for c in chunks:
    try:
        decoded = c.encode("utf-8").decode("unicode_escape")
        all_text += decoded + "\n"
    except:
        pass
print("Total decoded text length:", len(all_text))
print("Chunks count:", len(chunks))
keywords = ["TTL", "5 分", "5分", "分钟", "cache_control", "prefix", "读取", "写入", "创建", "命中", "限额", "配额", "quota", "plan", "过期", "失效", "重新计费", "Code Plan", "Token Plan", "CodePlan", "ephemeral", "Cache", "缓存"]
seen = set()
for keyword in keywords:
    for m in re.finditer(re.escape(keyword), all_text):
        start = max(0, m.start() - 150)
        end = min(len(all_text), m.end() + 250)
        snippet = all_text[start:end].replace("\n", " ")
        key = snippet[:80]
        if key in seen:
            continue
        seen.add(key)
        print("===", keyword, "===")
        print(snippet)
        print()