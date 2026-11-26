"""Applies the end-to-end edits (py/pick_edits.py manifest) to a COPY of the reference tree.

usage: python edit_tree.py <manifest.json> <tree copy> <tag> [--only <label substring>] [--skip <label substring>]
Never run it on the reference tree itself (the script refuses a path that holds `parity`).
Each edit is textual and keeps the file's BOM and line ends:
  bsl           a comment line `// dropin-e2e <tag>` at the end
  form_xml      the first non-empty <v8:content> text of the form gets ` [e2e <tag>]`
  template_xml  the same in a spreadsheet or data-composition template
  help_html     `<!-- dropin-e2e <tag> -->` before </body> (or at the end)
  template_bin  one zero byte at the end
Prints one line per edit; writes <tree copy>\\..\\edits_applied.json (sha256 before and after).
"""
import hashlib
import json
import os
import re
import sys


def sha(data):
    return hashlib.sha256(data).hexdigest()


def edit_bytes(kind, data, tag):
    bom = data.startswith(b'\xef\xbb\xbf')
    body = data[3:] if bom else data
    if kind == 'template_bin':
        return data + b'\x00'
    text = body.decode('utf-8')
    crlf = '\r\n' in text
    nl = '\r\n' if crlf else '\n'
    if kind == 'bsl':
        if not text.endswith(('\n', '\r\n')):
            text += nl
        text += '// dropin-e2e %s%s' % (tag, nl)
    elif kind in ('form_xml', 'template_xml'):
        match = None
        for candidate in re.finditer(r'(<v8:content>)([^<>&]+)(</v8:content>)', text):
            if candidate.group(2).strip():
                match = candidate
                break
        if match is None:
            raise ValueError('no plain <v8:content> text')
        new = match.group(1) + match.group(2) + ' [e2e %s]' % tag + match.group(3)
        text = text[:match.start()] + new + text[match.end():]
    elif kind == 'help_html':
        marker = '<!-- dropin-e2e %s -->' % tag
        index = text.rfind('</body>')
        if index >= 0:
            text = text[:index] + marker + text[index:]
        else:
            text += marker
    else:
        raise ValueError('unknown kind ' + kind)
    return (b'\xef\xbb\xbf' if bom else b'') + text.encode('utf-8')


def main():
    args = sys.argv[1:]
    only = skip = None
    if '--only' in args:
        i = args.index('--only'); only = args[i + 1]; del args[i:i + 2]
    if '--skip' in args:
        i = args.index('--skip'); skip = args[i + 1]; del args[i:i + 2]
    manifest, tree, tag = args[:3]
    if 'parity' in tree.lower() or not os.path.isdir(tree):
        sys.exit('refusing: %s' % tree)
    edits = json.load(open(manifest, encoding='utf-8'))
    applied = []
    for edit in edits:
        if only and only not in edit['label']:
            continue
        if skip and skip in edit['label']:
            continue
        path = os.path.join(tree, *edit['path'].split('/'))
        data = open(path, 'rb').read()
        try:
            new = edit_bytes(edit['kind'], data, tag)
        except ValueError as error:
            print('SKIP  %-48s %s (%s)' % (edit['label'], edit['path'], error))
            continue
        with open(path, 'wb') as f:
            f.write(new)
        applied.append({**edit, 'sha_before': sha(data), 'sha_after': sha(new), 'bytes_before': len(data), 'bytes_after': len(new)})
        print('EDIT  %-48s %s (%+d bytes)' % (edit['label'], edit['path'], len(new) - len(data)))
    out = os.path.join(os.path.dirname(tree.rstrip('\\/')), 'edits_applied.json')
    json.dump(applied, open(out, 'w', encoding='utf-8', newline='\n'), ensure_ascii=False, indent=1)
    print('%d edits applied; manifest %s' % (len(applied), out))


if __name__ == '__main__':
    main()
