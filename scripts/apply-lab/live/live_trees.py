#!/usr/bin/env python3
"""Source trees for the live-mode lab runs (#409 F-5, F-9, F-10): one common module each, from the reference export of the БСП 8.3.27 corpus.

usage: live_trees.py <output root> <tag> [<tag> ...]
  <root>/<tag>/CommonModules/ОбсужденияСлужебныйКлиентСервер/...   the value under "Telegram" of ТипыВнешнихСистем() is "LIVE-<tag>"

Every tag is a different module text, so a run with a new tag is never a no-op (a stage equal to the active rows is not promoted, and the
live switch is not run for it). The module is a client-server one with ExternalConnection = true, so a session can read the marker.
Each tree holds the owner XML and the module body of the reference export, byte for byte apart from the marker (BOM and CRLF as exported).
"""
import pathlib
import shutil
import sys

REF = pathlib.Path(r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native\CommonModules")
MODULE = "ОбсужденияСлужебныйКлиентСервер"
MARKER_LINE = 'ТипыИнтеграций.Вставить("Telegram", "Telegram");'.encode("utf-8")


def main() -> None:
    root = pathlib.Path(sys.argv[1])
    body = (REF / MODULE / "Ext" / "Module.bsl").read_bytes()
    assert body.count(MARKER_LINE) == 1, "the marker line is not unique"
    for tag in sys.argv[2:]:
        target = root / tag / "CommonModules"
        (target / MODULE / "Ext").mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REF / (MODULE + ".xml"), target / (MODULE + ".xml"))
        text = body.replace(MARKER_LINE, ('ТипыИнтеграций.Вставить("Telegram", "LIVE-%s");' % tag).encode("utf-8"))
        (target / MODULE / "Ext" / "Module.bsl").write_bytes(text)
        print("tree", tag, "->", target)


main()
