import os, glob, datetime, json, re
all_reqs = sorted(glob.glob('/tmp/xingdu_dumps/*_oai_req.json'), key=os.path.getmtime)
print('total oai req:', len(all_reqs))
print('=== last 3 dumps ===')
for f in all_reqs[-3:]:
    base = f[:-12]  # strip _oai_req.json
    anth_req = base + '_anth_req.json'
    oai_stream = base + '_oai_stream.txt'
    anth_sse = base + '_anth_sse.txt'
    mtime = os.path.getmtime(oai_stream) if os.path.exists(oai_stream) else 0
    ts = datetime.datetime.fromtimestamp(mtime).strftime('%H:%M:%S') if mtime else '?'
    name = os.path.basename(base)
    print('')
    print('==== %s @ %s ====' % (name, ts))
    print('oai_req: %d | anth_req: %d' % (os.path.getsize(f), os.path.getsize(anth_req) if os.path.exists(anth_req) else 0))
    print('oai_stream: %d | anth_sse: %d' % (os.path.getsize(oai_stream) if os.path.exists(oai_stream) else 0, os.path.getsize(anth_sse) if os.path.exists(anth_sse) else 0))
    with open(f, 'r', encoding='utf-8', errors='replace') as fh:
        try:
            data = json.load(fh)
            print('model: %s' % data.get('model', '?'))
            print('messages: %d, max_tokens: %s' % (len(data.get('messages', [])), data.get('max_tokens', '?')))
        except Exception as e:
            print('parse err: %s' % e)
    if os.path.exists(oai_stream):
        with open(oai_stream, 'r', encoding='utf-8', errors='replace') as fh:
            stream_content = fh.read()
        texts = re.findall(r'"text"\s*:\s*"([^"]*)"', stream_content)
        if texts:
            full_text = ''.join(texts)
            print('total text length: %d' % len(full_text))
            print('text head: %s' % full_text[:200])
            print('text tail: ...%s' % full_text[-200:])
        else:
            print('no text deltas found, raw stream:')
            print(stream_content[:500])