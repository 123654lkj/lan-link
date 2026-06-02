from flask import Flask, request, jsonify, Response
import requests as http_requests
import json, time, os, base64, re, copy

app = Flask(__name__)

BACKENDS = {
    'dashscope': {
        'url': 'https://coding.dashscope.aliyuncs.com/apps/anthropic/v1/messages',
        'api_key': 'sk-sp-eedb8e9310da40eab57bc573b6f3cd67',
        'models': ['qwen3.6-plus', 'qwen3.6', 'qwen-plus', 'qwen-max'],
        'label': 'DashScope',
    },
    'minimax': {
        'url': 'https://api.minimaxi.com/anthropic/v1/messages',
        'api_key': 'sk-cp-xd6FHMUfN6JslhCb2iaIE1v_MhMwDXxQfmiRxKzaZ76MdiNnuX8xa7o7nlpRQqXa8T8jouMx0lXKDeKLcFu7fYSpGEklosxZHWfex53oStaVxRur84H596E',
        'models': ['MiniMax-M3'],
        'label': 'MiniMax',
    },
}

MODEL_MAP = {}
for _bk, _bv in BACKENDS.items():
    for _m in _bv['models']:
        MODEL_MAP[_m] = _bk

DUMP_DIR = '/tmp/xingdu_dumps'
os.makedirs(DUMP_DIR, exist_ok=True)

MAX_MESSAGES = 200
AUTO_RETRY_ENABLED = True
AUTO_RETRY_MAX = 1
INTENT_PATTERNS = [
    r"(?:let me|I'll|I will|I should|I need to|I'm going to)\s+(?:check|run|execute|look|read|write|create|delete|search|find|install|update|start|open|try|build|compile|deploy|fix|modify)",
    r"(?:我来|让我|我将|我需要|接下来|现在)\s*(?:看|查|运行|执行|检查|搜索|安装|更新|写|创建|删除|试|修|改|编译|部署|启动)",
]

def get_backend(model):
    bk = MODEL_MAP.get(model)
    if bk:
        return BACKENDS[bk]
    return BACKENDS['dashscope']

def _oai_image_to_anthropic(block):
    image_url = block.get('image_url', {})
    url = image_url.get('url', '')
    if url.startswith('data:'):
        match = re.match(r'data:(image/[^;]+);base64,(.*)', url, re.DOTALL)
        if match:
            return {'type': 'image', 'source': {'type': 'base64', 'media_type': match.group(1), 'data': match.group(2)}}
    elif url.startswith('http://') or url.startswith('https://'):
        return {'type': 'image', 'source': {'type': 'url', 'url': url}}
    return None

def _oai_content_to_anthropic(content):
    if isinstance(content, str):
        return content if content else ''
    if isinstance(content, list):
        blocks = []
        for b in content:
            if not isinstance(b, dict):
                continue
            bt = b.get('type', '')
            if bt == 'text':
                blocks.append({'type': 'text', 'text': b.get('text', '')})
            elif bt == 'image_url':
                img = _oai_image_to_anthropic(b)
                if img:
                    blocks.append(img)
            elif bt == 'input_image':
                if b.get('data'):
                    blocks.append({'type': 'image', 'source': {'type': 'base64', 'media_type': b.get('media_type', 'image/png'), 'data': b['data']}})
        return blocks if blocks else ''
    return ''

def _merge_consecutive_roles(msgs):
    if not msgs:
        return msgs
    merged = [dict(msgs[0])]
    for msg in msgs[1:]:
        if msg['role'] == merged[-1]['role']:
            prev_content = merged[-1]['content']
            cur_content = msg['content']
            if isinstance(prev_content, str) and isinstance(cur_content, str):
                merged[-1] = dict(merged[-1])
                merged[-1]['content'] = prev_content + '\n' + cur_content
            elif isinstance(prev_content, list) and isinstance(cur_content, list):
                merged[-1] = dict(merged[-1])
                merged[-1]['content'] = prev_content + cur_content
            else:
                p = prev_content if isinstance(prev_content, list) else [{'type': 'text', 'text': prev_content}]
                c = cur_content if isinstance(cur_content, list) else [{'type': 'text', 'text': cur_content}]
                merged[-1] = dict(merged[-1])
                merged[-1]['content'] = p + c
        else:
            merged.append(dict(msg))
    return merged

def _truncate_messages(msgs, max_msgs):
    if len(msgs) <= max_msgs:
        return msgs
    head = msgs[:2]
    tail = msgs[-(max_msgs - 2):]
    print('[TRUNCATE] %d -> %d messages' % (len(msgs), len(head) + len(tail)), flush=True)
    return head + tail

def _has_intent_text(text):
    if not text or len(text) < 50:
        return False
    for pat in INTENT_PATTERNS:
        if re.search(pat, text, re.IGNORECASE):
            return True
    return False

@app.route('/v1/chat/completions', methods=['POST'])
def chat_completions():
    body = request.get_data()
    try:
        oai = json.loads(body)
    except Exception:
        oai = {}
    stream = oai.get('stream', False)
    model = oai.get('model', 'qwen3.6-plus')
    msgs = oai.get('messages', [])
    oai_tools = oai.get('tools', [])
    ts = str(int(time.time()))
    print('[REQ] %s model=%s msgs=%d stream=%s has_tools=%s' % (ts, model, len(msgs), stream, bool(oai_tools)), flush=True)
    with open(os.path.join(DUMP_DIR, ts + '_oai_req.json'), 'wb') as f:
        f.write(body)

    backend = get_backend(model)
    anth_url = backend['url']
    api_key = backend['api_key']
    print('[BACKEND] %s -> %s' % (model, backend['label']), flush=True)

    system_parts = []
    anth_msgs = []
    for msg in msgs:
        role = msg.get('role', '')
        content = msg.get('content', '')
        tool_calls = msg.get('tool_calls', [])
        tool_call_id = msg.get('tool_call_id', None)
        name = msg.get('name', None)
        if role == 'system':
            system_parts.append(content if isinstance(content, str) else str(content))
        elif role == 'tool':
            tc_text = content if isinstance(content, str) else json.dumps(content, ensure_ascii=False)
            anth_msgs.append({'role': 'user', 'content': [{'type': 'tool_result', 'tool_use_id': tool_call_id or name or 'unknown', 'content': tc_text}]})
        elif role == 'assistant' and tool_calls:
            blocks = []
            if content:
                blocks.append({'type': 'text', 'text': content if isinstance(content, str) else str(content)})
            for tc in tool_calls:
                fn = tc.get('function', {})
                try:
                    inp = json.loads(fn.get('arguments', '{}'))
                except Exception:
                    inp = {}
                blocks.append({'type': 'tool_use', 'id': tc.get('id', ''), 'name': fn.get('name', ''), 'input': inp})
            anth_msgs.append({'role': 'assistant', 'content': blocks})
        elif role in ('user', 'assistant'):
            anth_content = _oai_content_to_anthropic(content)
            if role == 'user' and isinstance(anth_content, list):
                anth_msgs.append({'role': 'user', 'content': anth_content})
            else:
                anth_msgs.append({'role': role, 'content': anth_content if anth_content else ''})

    orig_count = len(anth_msgs)
    anth_msgs = _merge_consecutive_roles(anth_msgs)
    if len(anth_msgs) != orig_count:
        print('[MERGE] %d -> %d messages' % (orig_count, len(anth_msgs)), flush=True)
    anth_msgs = _truncate_messages(anth_msgs, MAX_MESSAGES)

    anth_tools = []
    for t in oai_tools:
        f = t.get('function', {})
        anth_tools.append({'name': f.get('name', ''), 'description': f.get('description', ''), 'input_schema': f.get('parameters', {'type': 'object', 'properties': {}})})

    max_tokens = oai.get('max_tokens') or oai.get('max_completion_tokens') or 16384
    payload = {'model': model, 'max_tokens': max_tokens, 'messages': anth_msgs, 'stream': stream}

    xingdu_sys_parts = [
        'CRITICAL BEHAVIOR RULES:',
        '1. When you decide to take an action, IMMEDIATELY call the relevant tool function. Never just describe what you plan to do.',
        '2. If you think "I should run a command" -> call exec_command with the command.',
        '3. If you think "I should read a file" -> call the appropriate tool to read it.',
        '4. If you think "I should write/modify code" -> call the tool to do it.',
        '5. NEVER end your response with just an intention like "Let me..." or "I will..." without actually calling the tool.',
        '6. Think deeply before acting, but ALWAYS act when you have a plan.',
    ]
    if anth_tools:
        tool_names = [t['name'] for t in anth_tools]
        xingdu_sys_parts.append('Available tools: ' + ', '.join(tool_names))
        xingdu_sys_parts.append('IMPORTANT: You MUST use these tools to execute actions. Do NOT just describe your plans in text.')

    system_parts.insert(0, '\n'.join(xingdu_sys_parts))
    if system_parts:
        payload['system'] = '\n'.join(system_parts)
    if anth_tools:
        payload['tools'] = anth_tools

    tc = oai.get('tool_choice')
    if tc is not None:
        if isinstance(tc, str):
            if tc == 'auto':
                payload['tool_choice'] = {'type': 'auto'}
            elif tc == 'none':
                payload['tool_choice'] = {'type': 'none'}
            elif tc == 'required':
                payload['tool_choice'] = {'type': 'any'}
        elif isinstance(tc, dict):
            fn = tc.get('function', {})
            if fn.get('name'):
                payload['tool_choice'] = {'type': 'tool', 'name': fn['name']}
            elif tc.get('type') == 'required':
                payload['tool_choice'] = {'type': 'any'}

    if model in ('MiniMax-M3',):
        payload['thinking'] = {'type': 'adaptive'}

    for param in ('temperature', 'top_p'):
        if param in oai:
            payload[param] = oai[param]
    if 'stop' in oai:
        stops = oai['stop']
        if isinstance(stops, str):
            stops = [stops]
        payload['stop_sequences'] = stops

    with open(os.path.join(DUMP_DIR, ts + '_anth_req.json'), 'w') as f:
        json.dump(payload, f, ensure_ascii=False)
    print('[ANTH] %s %s sys=%d msgs=%d/%d tools=%d t=%s tp=%s stop=%s max_tokens=%d' % (
        ts, backend['label'], len(system_parts), len(anth_msgs), len(msgs), len(anth_tools),
        payload.get('temperature'), payload.get('top_p'), payload.get('stop_sequences'), max_tokens), flush=True)

    hdrs = {'Content-Type': 'application/json', 'x-api-key': api_key, 'anthropic-version': '2023-06-01'}
    if stream:
        return Response(_do_stream_with_retry(anth_url, payload, hdrs, model, ts, anth_tools), content_type='text/event-stream')

    resp = http_requests.post(anth_url, json=payload, headers=hdrs, timeout=120)
    print('[RESP] %s status=%d' % (ts, resp.status_code), flush=True)
    if resp.status_code != 200:
        print('[ERROR] ' + resp.text[:500], flush=True)
        return jsonify({'error': resp.text}), resp.status_code
    result = resp.json()

    if AUTO_RETRY_ENABLED and anth_tools and _is_lazy_response(result):
        print('[RETRY] %s non-stream lazy response detected, retrying...' % ts, flush=True)
        retry_payload = copy.deepcopy(payload)
        retry_payload['messages'].append({'role': 'assistant', 'content': result.get('content', [])})
        retry_payload['messages'].append({'role': 'user', 'content': 'Execute the actions you described above using the available tools. Call the tool functions directly.'})
        retry_payload['messages'] = _merge_consecutive_roles(retry_payload['messages'])
        resp2 = http_requests.post(anth_url, json=retry_payload, headers=hdrs, timeout=120)
        if resp2.status_code == 200:
            result = resp2.json()
            print('[RETRY] %s success' % ts, flush=True)

    out = _anth_to_oai(result, model)
    with open(os.path.join(DUMP_DIR, ts + '_oai_resp.json'), 'w') as f:
        json.dump(out, f, ensure_ascii=False)
    return jsonify(out)

def _is_lazy_response(anth_result):
    content = anth_result.get('content', [])
    stop_reason = anth_result.get('stop_reason', '')
    if stop_reason == 'tool_use':
        return False
    text = ''
    has_tools = False
    for b in content:
        if b.get('type') == 'text':
            text += b.get('text', '')
        elif b.get('type') == 'tool_use':
            has_tools = True
    if has_tools:
        return False
    return _has_intent_text(text)

def _anth_to_oai(ar, model):
    content, tool_calls, reasoning = '', [], ''
    for b in ar.get('content', []):
        bt = b.get('type', '')
        if bt == 'text':
            content += b.get('text', '')
        elif bt == 'tool_use':
            tool_calls.append({'id': b['id'], 'type': 'function', 'function': {'name': b['name'], 'arguments': json.dumps(b.get('input', {}))}})
        elif bt == 'thinking':
            reasoning += b.get('thinking', '')
    msg = {'role': 'assistant', 'content': content}
    if tool_calls:
        msg['tool_calls'] = tool_calls
    if reasoning:
        msg['reasoning_content'] = reasoning
    fr = 'tool_calls' if ar.get('stop_reason') == 'tool_use' else 'stop'
    u = ar.get('usage', {})
    return {'id': ar.get('id', 'chatcmpl-xingdu'), 'object': 'chat.completion', 'created': int(time.time()), 'model': model,
        'choices': [{'index': 0, 'message': msg, 'finish_reason': fr}],
        'usage': {'prompt_tokens': u.get('input_tokens', 0), 'completion_tokens': u.get('output_tokens', 0), 'total_tokens': u.get('input_tokens', 0) + u.get('output_tokens', 0)}}

def _do_stream_with_retry(anth_url, payload, hdrs, model, ts, anth_tools):
    buffered_events = []
    full_text = ''
    full_thinking = ''
    has_tool_calls = False
    finish_reason = None

    try:
        resp = http_requests.post(anth_url, json=payload, headers=hdrs, stream=True, timeout=120)
        print('[STREAM] %s status=%d' % (ts, resp.status_code), flush=True)
        if resp.status_code != 200:
            err = resp.text[:500]
            print('[STREAM ERROR] ' + err, flush=True)
            chunk = {'id': 'chatcmpl-xingdu-err', 'object': 'chat.completion.chunk', 'created': int(time.time()), 'model': model, 'choices': [{'index': 0, 'delta': {'content': '[error] ' + err}, 'finish_reason': None}]}
            yield 'data: ' + json.dumps(chunk) + '\n\n'
            yield 'data: [DONE]\n\n'
            return

        for line in resp.iter_lines():
            if not line:
                continue
            ls = line.decode('utf-8')
            if not ls.startswith('data:'):
                continue
            ed = ls[5:].strip()
            if ed == '[DONE]':
                continue
            try:
                evt = json.loads(ed)
            except Exception:
                continue
            with open(os.path.join(DUMP_DIR, ts + '_anth_sse.txt'), 'a') as _df:
                _df.write(ls + '\n')
            et = evt.get('type', '')
            if et == 'content_block_delta':
                d = evt.get('delta', {})
                dt = d.get('type', '')
                if dt == 'text_delta':
                    full_text += d.get('text', '')
                elif dt == 'thinking_delta':
                    full_thinking += d.get('thinking', '')
                elif dt == 'input_json_delta':
                    has_tool_calls = True
            elif et == 'content_block_start':
                cb = evt.get('content_block', {})
                if cb.get('type') == 'tool_use':
                    has_tool_calls = True
            elif et == 'message_delta':
                sr = evt.get('delta', {}).get('stop_reason', '')
                finish_reason = 'tool_calls' if sr == 'tool_use' else 'stop'
            out = _stream_event(evt, model)
            if out:
                buffered_events.append(out)

    except Exception as e:
        print('[STREAM EXC] ' + str(e), flush=True)
        chunk = {'id': 'chatcmpl-xingdu-err', 'object': 'chat.completion.chunk', 'created': int(time.time()), 'model': model, 'choices': [{'index': 0, 'delta': {'content': '[stream error] ' + str(e)}, 'finish_reason': None}]}
        yield 'data: ' + json.dumps(chunk) + '\n\n'
        yield 'data: [DONE]\n\n'
        return

    is_lazy = (AUTO_RETRY_ENABLED and anth_tools and not has_tool_calls
               and finish_reason == 'stop' and _has_intent_text(full_text))

    if is_lazy and AUTO_RETRY_MAX > 0:
        print('[RETRY-STREAM] %s lazy detected (text=%d chars, no tools), retrying...' % (ts, len(full_text)), flush=True)
        retry_payload = copy.deepcopy(payload)
        retry_payload['messages'].append({'role': 'assistant', 'content': [
            {'type': 'text', 'text': full_text}
        ]})
        if full_thinking:
            retry_payload['messages'][-1]['content'].insert(0, {'type': 'thinking', 'thinking': full_thinking})
        retry_payload['messages'].append({'role': 'user', 'content': 'IMPORTANT: You described actions above but did not execute them. Now EXECUTE those actions by calling the relevant tool functions. Do not repeat your analysis - just call the tools.'})
        retry_payload['messages'] = _merge_consecutive_roles(retry_payload['messages'])
        retry_payload['messages'] = _truncate_messages(retry_payload['messages'], MAX_MESSAGES)

        try:
            resp2 = http_requests.post(anth_url, json=retry_payload, headers=hdrs, stream=True, timeout=120)
            if resp2.status_code == 200:
                retry_events = []
                for line in resp2.iter_lines():
                    if not line:
                        continue
                    ls = line.decode('utf-8')
                    if not ls.startswith('data:'):
                        continue
                    ed = ls[5:].strip()
                    if ed == '[DONE]':
                        continue
                    try:
                        evt = json.loads(ed)
                    except Exception:
                        continue
                    out = _stream_event(evt, model)
                    if out:
                        retry_events.append(out)
                if retry_events:
                    print('[RETRY-STREAM] %s success, %d events' % (ts, len(retry_events)), flush=True)
                    with open(os.path.join(DUMP_DIR, ts + '_oai_stream.txt'), 'w') as f:
                        f.writelines(retry_events)
                    for evt in retry_events:
                        yield evt
                    yield 'data: [DONE]\n\n'
                    return
                else:
                    print('[RETRY-STREAM] %s no events from retry, using original' % ts, flush=True)
            else:
                print('[RETRY-STREAM] %s retry failed status=%d, using original' % (ts, resp2.status_code), flush=True)
        except Exception as e:
            print('[RETRY-STREAM] %s retry exception: %s, using original' % (ts, str(e)), flush=True)

    with open(os.path.join(DUMP_DIR, ts + '_oai_stream.txt'), 'w') as f:
        f.writelines(buffered_events)
    for evt in buffered_events:
        yield evt
    yield 'data: [DONE]\n\n'

def _stream_event(evt, model):
    t = evt.get('type', '')
    ts_val = int(time.time())
    NL = '\n\n'
    if t == 'content_block_delta':
        d = evt.get('delta', {})
        dt = d.get('type', '')
        if dt == 'text_delta':
            return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {'content': d.get('text', '')}, 'finish_reason': None}]}) + NL
        elif dt == 'input_json_delta':
            idx = evt.get('index', 0)
            return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {'tool_calls': [{'index': idx, 'function': {'arguments': d.get('partial_json', '')}}]}, 'finish_reason': None}]}) + NL
        elif dt == 'thinking_delta':
            return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {'reasoning_content': d.get('thinking', '')}, 'finish_reason': None}]}) + NL
        return None
    elif t == 'content_block_start':
        cb = evt.get('content_block', {})
        cbt = cb.get('type', '')
        if cbt == 'tool_use':
            idx = evt.get('index', 0)
            return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {'tool_calls': [{'index': idx, 'id': cb.get('id', ''), 'type': 'function', 'function': {'name': cb.get('name', ''), 'arguments': ''}}]}, 'finish_reason': None}]}) + NL
        elif cbt == 'thinking':
            return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {'reasoning_content': ''}, 'finish_reason': None}]}) + NL
        return None
    elif t == 'message_delta':
        sr = evt.get('delta', {}).get('stop_reason', '')
        fr = 'tool_calls' if sr == 'tool_use' else 'stop'
        return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {}, 'finish_reason': fr}]}) + NL
    elif t == 'message_start':
        return 'data: ' + json.dumps({'id': 'chatcmpl-xingdu', 'object': 'chat.completion.chunk', 'created': ts_val, 'model': model, 'choices': [{'index': 0, 'delta': {'role': 'assistant', 'content': ''}, 'finish_reason': None}]}) + NL
    return None

@app.route('/v1/models', methods=['GET'])
def list_models():
    models = []
    for bk, bv in BACKENDS.items():
        for m in bv['models']:
            models.append({'id': m, 'object': 'model', 'owned_by': bk})
    return jsonify({'object': 'list', 'data': models})

@app.route('/health', methods=['GET'])
def health():
    return jsonify({'status': 'ok', 'backends': list(BACKENDS.keys())})

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=9999)