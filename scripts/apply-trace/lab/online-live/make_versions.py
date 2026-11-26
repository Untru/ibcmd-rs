"""Builds the marker source trees src/<tag>/ for the #344 runs.

Each tree holds the owner XML and the module body of ONE existing common module
(client+server, CommonModules/ОбсужденияСлужебныйКлиентСервер) whose function
ТипыВнешнихСистем() returns a structure; the marker is the value stored under the
key "Telegram". The body is byte-identical to the reference export except that
value (CRLF and BOM as in the export). The observer reads
ТипыВнешнихСистем().Telegram on the client and on the server.

usage: python make_versions.py <tag>=<marker> [<tag>=<marker> ...]
       (marker "-" keeps the original value "Telegram")
"""
import pathlib
import shutil
import sys

REF = pathlib.Path(r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native\CommonModules")
NAME = "ОбсужденияСлужебныйКлиентСервер"
OUT = pathlib.Path(r"F:\ibcmd\lab\05\online\src")

body = (REF / NAME / "Ext" / "Module.bsl").read_bytes()
needle = 'ТипыИнтеграций.Вставить("Telegram", "Telegram");'.encode("utf-8")
assert body.count(needle) == 1, "the marker line is not unique"

for spec in sys.argv[1:]:
    tag, marker = spec.split("=", 1)
    root = OUT / tag / "CommonModules"
    (root / NAME / "Ext").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(REF / (NAME + ".xml"), root / (NAME + ".xml"))
    if marker == "-":
        text = body
    else:
        text = body.replace(needle, 'ТипыИнтеграций.Вставить("Telegram", "{}");'.format(marker).encode("utf-8"))
    (root / NAME / "Ext" / "Module.bsl").write_bytes(text)
    print(tag, marker, len(text))
