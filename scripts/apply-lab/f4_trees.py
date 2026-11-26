#!/usr/bin/env python3
"""Source trees of the F-4 repro (#408): one common module each, from the reference export of the БСП 8.3.27 corpus.

usage: f4_trees.py <output root>
  <root>/g2/CommonModules/ОбсужденияСлужебныйКлиентСервер/...   the online generation: the value under "Telegram" of
                                                                ТипыВнешнихСистем() is "G2MARK" (a session reads it)
  <root>/x1/CommonModules/РаботаСКлассификаторамиКлиентСервер/... the exclusive change: the module gets the function
                                                                ПроверкаIbcmdRsF4() that returns "F4"
Both modules have ExternalConnection = true, so an external-connection session (COM) can call them. Each tree holds the
owner XML and the module body of the reference export, byte for byte apart from the edit (BOM and CRLF as in the export).
"""
import pathlib
import shutil
import sys

REF = pathlib.Path(r"F:\ibcmd\lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_export_recheck_2\native\CommonModules")
G2_MODULE = "ОбсужденияСлужебныйКлиентСервер"
X1_MODULE = "РаботаСКлассификаторамиКлиентСервер"
MARKER_LINE = 'ТипыИнтеграций.Вставить("Telegram", "Telegram");'.encode("utf-8")
PROBE = (
    "\r\n\r\nФункция ПроверкаIbcmdRsF4() Экспорт\r\n\tВозврат \"F4\";\r\nКонецФункции\r\n"
).encode("utf-8")


def tree(root: pathlib.Path, name: str, body: bytes) -> None:
    target = root / "CommonModules"
    (target / name / "Ext").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(REF / (name + ".xml"), target / (name + ".xml"))
    (target / name / "Ext" / "Module.bsl").write_bytes(body)


def main() -> None:
    root = pathlib.Path(sys.argv[1])
    g2 = (REF / G2_MODULE / "Ext" / "Module.bsl").read_bytes()
    assert g2.count(MARKER_LINE) == 1, "the marker line is not unique"
    tree(root / "g2", G2_MODULE, g2.replace(MARKER_LINE, 'ТипыИнтеграций.Вставить("Telegram", "G2MARK");'.encode("utf-8")))
    x1 = (REF / X1_MODULE / "Ext" / "Module.bsl").read_bytes()
    tree(root / "x1", X1_MODULE, x1.rstrip(b"\r\n") + PROBE)
    print("trees written to", root)


main()
