# Observed installed EDT limitations

These observations were reproduced on 2026-10-01 with installed EDT
2025.2.3.30 and the genuine ERP UH 8.3.27 corpus. They describe the source
tool, not a successful ibcmd-rs conversion. The original files and all raw
captures remain in the F: laboratory.

## Empty converted mobile signature

The native `Ext/MobileClientSignature.bin` and authentic EDT
`Configuration/MobileClientSign.bin` contain the same 44-byte version-2
artifact. The installed reader reports `Unsupported version of file
MobileDigiSign.bin`. A bounded read-only investigation reproduces its text-list
tail failure; no supported equivalent framing was found. Replacing it with a
basic version-0 object changes the model class and is not an accepted repair.

The exact SDK jars, source bytes, probe commands and unchanged-source hashes
are bound under `F:\ibcmd\lab\07\mobile-signature-reader-probe`.
[The oracle procedure](oracle.md) preserves this error outside the approved
empty-project environment diagnostics. A zero-byte validation TSV does not
waive it.

## Transparency on a particular picture reference

The native form
`Catalogs/ДокументыБД/Forms/ФормаВыбора/Ext/Form.xml` has a header picture
reference to `CommonPicture.УправлениеПроцессом` with `LoadTransparent=true`
and pixel `(12,12)`. The authentic EDT form carries the picture reference;
the post-EDT native SDK export writes `LoadTransparent=false` and no pixel.
The referenced CommonPicture metadata itself has no corresponding pixel,
so deriving the per-use setting from that metadata is incorrect.

The complete selected-file hashes and observed header elements are preserved
in `F:\ibcmd\lab\07\uha83-per-use-transparency-loss.json`, bound to the fresh
native SDK capture. The original and post-EDT form SHA-256 values are
`a18eeaa18502bdc78a4e17be2fe901e4b936d5daca346192dca6b94d8ea9c588`
and `62673c0b0bb93d16380c439a1bdb2ff79c76dd586de857b3807889c60ac2d0e3`.
The raw tree comparisons retain this difference.

Native reader/writer support must retain the typed per-use flag and pixel.
An EDT projection that cannot represent them must reject the conversion
before publication. Keeping original XML for an unchanged return does not
establish that EDT can import those source semantics without loss.

## Acceptance scope

The comparison bench can record SDK failures and source-tool losses on UH.
That evidence is distinct from a clean generated-project import or a full
conversion PASS. Issue #356 specifies BSP native-export equality; issue #357
requires an importable generated project and exact unchanged XML return.
Neither criterion is satisfied by ignoring unsupported source artifacts.
