# Observed installed EDT limitations

These observations were reproduced on 2026-10-01 with installed EDT
2025.2.3.30 and the genuine ERP UH 8.3.27 corpus. They describe the source
tool, not a successful ibcmd-rs conversion. The original files and all raw
captures remain in the F: laboratory.

## Empty converted mobile signature

The native `Ext/MobileClientSignature.bin` and authentic EDT
`Configuration/MobileClientSign.bin` contain the same 44-byte version-2
artifact. The installed reader reports `Unsupported version of file
MobileDigiSign.bin`. Investigation reproduced its text-list tail failure.
The subsequent [model-bound version-2 transport](mobile-signature.md) preserves
the complete version-2 model through the installed reader and writer, including
nonempty digest groups and quoted text. Replacing it with a version-0 object
would change the model class and remains an invalid repair.

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

The adapter now retains the typed per-use flag and pixel through an explicit
[versioned form resource](picture-semantics.md), independent of unchanged-source
provenance. Descriptor references remain compatible with the installed EDT
model. The adapter reads this resource when converting back to native XML;
an EDT-only export does not preserve the additional per-use values. Installed
EDT import and validation of the generated resource remain separate acceptance
checks. This is a transport for the missing semantics, rather than a reason to
exclude the configuration.

## Acceptance scope

The comparison bench can record SDK failures and source-tool losses on UH.
That evidence is distinct from a clean generated-project import or a full
conversion PASS. Issue #356 specifies BSP native-export equality; issue #357
requires an importable generated project and exact unchanged XML return.
Neither criterion is satisfied by ignoring unsupported source artifacts.
