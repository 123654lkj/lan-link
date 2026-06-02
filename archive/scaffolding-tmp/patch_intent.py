import sys, os
sys.stdout.reconfigure(encoding="utf-8")

path = "/opt/dashscope_proxy.py"
# Read current file from local copy
with open(r"G:\codex-AI-tools\tmp\xingdu_current.py", "r", encoding="utf-8") as f:
    content = f.read()

# Replace INTENT_PATTERNS and _has_intent_text with broader substring matching
old_intent = '''INTENT_PATTERNS = [
    r"(?:let me|I'll|I will|I should|I need to|I'm going to)\\s+(?:check|run|execute|look|read|write|create|delete|search|find|install|update|start|open|try|build|compile|deploy|fix|modify)",
    r"(?:我来|让我|我将|我需要|接下来|现在)\\s*(?:看|查|运行|执行|检查|搜索|安装|更新|写|创建|删除|试|修|改|编译|部署|启动)",
]'''

new_intent = '''INTENT_PATTERNS_EN = [
    "let me", "i will", "i should", "i need to", "i have to",
    "i must", "i'm going to", "i can", "the next step",
    "i'll ", "going to ", "now i ", "then i ",
]
INTENT_PATTERNS_ZH = [
    "让我", "我得", "我将", "我需要", "接下来",
    "现在", "我来", "我要", "先", "开始",
    "继续", "第一步", "首先", "然后",
]'''

content = content.replace(old_intent, new_intent)

# Replace _has_intent_text function
old_func = '''def _has_intent_text(text):
    if not text or len(text) < 50:
        return False
    for pat in INTENT_PATTERNS:
        if re.search(pat, text, re.IGNORECASE):
            return True
    return False'''

new_func = '''def _has_intent_text(text):
    """Detect action-intent in text using simple substring matching."""
    if not text or len(text) < 30:
        return False
    lower = text.lower()
    for kw in INTENT_PATTERNS_EN:
        if kw in lower:
            return True
    for kw in INTENT_PATTERNS_ZH:
        if kw in text:
            return True
    return False'''

content = content.replace(old_func, new_func)

outpath = r"G:\codex-AI-tools\tmp\dashscope_proxy_v3.py"
with open(outpath, "w", encoding="utf-8", newline="\n") as f:
    f.write(content)
print("Patched. Size: %d bytes" % len(content))

# Verify patches applied
if new_func in content:
    print("OK: _has_intent_text patched")
else:
    print("FAIL: _has_intent_text not found")
if new_intent[:30] in content:
    print("OK: INTENT_PATTERNS patched")
else:
    print("FAIL: INTENT_PATTERNS not found")