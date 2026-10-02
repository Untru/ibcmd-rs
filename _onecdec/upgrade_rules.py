"""Derive the platform's upgrade of older stored records from pairs: a
container as stored (older platform) and the same configuration re-serialized
by 8.3.27 (`reserialize.py`: its native XML loaded back and saved).

    python upgrade_rules.py <old.cf> <new.cf> [--show N]

Every brace list is keyed by its version tag (its first member when that is a
number). Where an old list and its new counterpart differ, the difference is
reduced to a rule: (old tag, old length) -> (new tag, new length) with the
members inserted (position, value) and removed. Rules are counted over all
rows; a rule seen with one value everywhere is a candidate for a static
upgrade.
"""
import collections, os, sys, zlib
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import v8c


def parse(text, i=0):
    """Brace text -> nested lists of str leaves; returns (node, next index)."""
    assert text[i] == '{'
    items, cur, i = [], [], i + 1
    quoted = False
    while i < len(text):
        ch = text[i]
        if quoted:
            cur.append(ch)
            if ch == '"':
                if i + 1 < len(text) and text[i + 1] == '"':
                    cur.append('"')
                    i += 1
                else:
                    quoted = False
        elif ch == '"':
            quoted = True
            cur.append(ch)
        elif ch == '{':
            node, i = parse(text, i)
            items.append(node)
            cur = None
            i -= 1
        elif ch in ',}':
            if cur is not None:
                items.append(''.join(cur).strip())
            elif ch == ',' and items and isinstance(items[-1], list) and False:
                pass
            if ch == '}':
                return items, i + 1
            cur = []
        elif ch in '\r\n':
            pass
        else:
            if cur is None:
                cur = []
            cur.append(ch)
        i += 1
    raise ValueError('unterminated')


def render(node):
    if isinstance(node, list):
        return '{' + ','.join(render(n) for n in node) + '}'
    return node


def tag(node):
    if isinstance(node, list) and node and isinstance(node[0], str) and node[0].lstrip('-').isdigit():
        return node[0]
    return None


def lcs(a, b):
    ka, kb = [render(x) for x in a], [render(x) for x in b]
    n, m = len(ka), len(kb)
    dp = [[0] * (m + 1) for _ in range(n + 1)]
    for i in range(n - 1, -1, -1):
        for j in range(m - 1, -1, -1):
            dp[i][j] = dp[i + 1][j + 1] + 1 if ka[i] == kb[j] else max(dp[i + 1][j], dp[i][j + 1])
    pairs, i, j = [], 0, 0
    while i < n and j < m:
        if ka[i] == kb[j]:
            pairs.append((i, j)); i += 1; j += 1
        elif dp[i + 1][j] >= dp[i][j + 1]:
            i += 1
        else:
            j += 1
    return pairs


def compare(old, new, rules, context):
    if render(old) == render(new):
        return
    if not (isinstance(old, list) and isinstance(new, list)):
        rules[('leaf', context, render(old)[:40], render(new)[:40])] += 1
        return
    key = (tag(old), len(old), tag(new), len(new), context)
    if len(old) == len(new):
        for idx, (a, b) in enumerate(zip(old, new)):
            compare(a, b, rules, (tag(old), tag(new), len(old), idx))
        return
    pairs = lcs(old, new)
    # Unmatched runs between anchors: pair positionally, the rest are
    # insertions (new) or removals (old).
    anchors = [(-1, -1)] + pairs + [(len(old), len(new))]
    inserted, removed = [], []
    for (ia, ja), (ib, jb) in zip(anchors, anchors[1:]):
        olds, news = list(range(ia + 1, ib)), list(range(ja + 1, jb))
        common = min(len(olds), len(news))
        for k in range(common):
            compare(old[olds[k]], new[news[k]], rules, (tag(old), tag(new), len(old), olds[k]))
        for j in news[common:]:
            inserted.append((j, render(new[j])[:60]))
        for i in olds[common:]:
            removed.append((i, render(old[i])[:60]))
    rules[('list',) + key + (tuple(inserted), tuple(removed))] += 1


def rows(path):
    c = v8c.V8(open(path, 'rb').read())
    out = {}
    for name, da in c.entries():
        raw = c.block(da)
        try:
            body = zlib.decompress(raw, -15)
        except zlib.error:
            continue
        if body[:4] == b'\xff\xff\xff\x7f':
            continue
        try:
            text = body.decode('utf-8-sig')
        except UnicodeDecodeError:
            continue
        if text.lstrip().startswith('{'):
            out[name] = text
    return out


def main():
    old_path, new_path = sys.argv[1], sys.argv[2]
    show = int(sys.argv[sys.argv.index('--show') + 1]) if '--show' in sys.argv else 80
    old, new = rows(old_path), rows(new_path)
    rules = collections.Counter()
    same = differ = failed = 0
    by_code = collections.Counter()
    for name in sorted(set(old) & set(new)):
        if old[name] == new[name]:
            same += 1
            continue
        try:
            a, _ = parse(old[name], old[name].index('{'))
            b, _ = parse(new[name], new[name].index('{'))
        except Exception:
            failed += 1
            continue
        differ += 1
        compare(a, b, rules, ('row',))
    print('rows same %d differ %d unparsed %d only-old %d only-new %d' % (
        same, differ, failed, len(set(old) - set(new)), len(set(new) - set(old))))
    for rule, count in rules.most_common(show):
        print('%6d %s' % (count, rule))


if __name__ == '__main__':
    main()
