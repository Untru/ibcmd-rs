"""SPIKE (throwaway): feed an .epf to `ibcmd-rs cf export` as if it were a cf that
holds one DataProcessor, to see what the stock pipeline makes of it.

    python spike_adapter.py <in.epf> <out.cf>
"""
import sys, re, uuid
import v8c

COMMANDS = '{45556acb-826a-4f73-898a-6025fc9536e1,0}'


def fields(t, i):
    """top-level comma-separated field spans of the brace list opening at t[i]=='{'"""
    assert t[i] == '{'
    depth, start, out, j, q = 0, i + 1, [], i, False
    while j < len(t):
        c = t[j]
        if q:
            if c == '"':
                if j + 1 < len(t) and t[j + 1] == '"': j += 1
                else: q = False
        elif c == '"': q = True
        elif c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                out.append((start, j)); return out, j
        elif c == ',' and depth == 1:
            out.append((start, j)); start = j + 1
        j += 1
    raise ValueError('unbalanced')


def adapt(main_text):
    t = main_text
    top, _ = fields(t, t.index('{'))
    # {1,{<main>},1,{<class>,{1,{4,...},N,colls}}}
    wrap = t[top[3][0]:top[3][1]].strip()
    wf, _ = fields(wrap, 0)
    class_id = wrap[wf[0][0]:wf[0][1]].strip()
    inner = wrap[wf[1][0]:wf[1][1]].strip()
    inf, _ = fields(inner, 0)
    header = inner[inf[1][0]:inf[1][1]].strip()
    hf, _ = fields(header, 0)
    parts = [header[a:b].strip() for a, b in hf]
    assert parts[0] == '4', parts[0]
    t1, t2, names, deff, blank, aux = parts[1:7]
    obj = re.search(r'\{1,0,([0-9a-f-]{36})\}', names).group(1)
    m1, m2 = str(uuid.uuid5(uuid.NAMESPACE_URL, 'mgr/' + t1)), str(uuid.uuid5(uuid.NAMESPACE_URL, 'mgr/' + t2))
    dp_header = '{17,%s,%s,\r\n%s,%s,1,0,%s,%s,%s,\r\n{0},\r\n{0}\r\n}' % (t1, t2, names, deff, m1, m2, aux)
    n = int(inner[inf[2][0]:inf[2][1]])
    colls = [inner[a:b].strip() for a, b in inf[3:]]
    colls.insert(2, COMMANDS)
    dp = '{1,\r\n%s,%d,\r\n%s\r\n}' % (dp_header, n + 1, ',\r\n'.join(colls))
    return class_id, obj, dp


def main():
    e = v8c.read(sys.argv[1])
    root = e['root'].decode('utf-8-sig')
    main_id = root.split(',')[1].strip()
    cls, obj, dp = adapt(e[main_id].decode('utf-8-sig'))
    print('class', cls, 'main', main_id, 'object', obj)
    out = {k: v for k, v in e.items() if k not in (main_id, 'copyinfo')}
    out[obj] = b'\xef\xbb\xbf' + dp.encode('utf-8')
    v = e['versions'].decode('utf-8-sig').replace('"%s"' % main_id, '"%s"' % obj)
    v = re.sub(r',\s*"copyinfo",\s*[0-9a-f-]{36}', '', v)
    n = len(re.findall(r'"[^"]*",\s*[0-9a-f-]{36}', v))
    v = re.sub(r'^\{1,\d+,', '{1,%d,' % n, v)
    out['versions'] = b'\xef\xbb\xbf' + v.encode('utf-8')
    open(sys.argv[2], 'wb').write(v8c.write15(out))


if __name__ == '__main__':
    main()
