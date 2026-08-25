# Open lead: module bodies with no V8 container framing, 20260825

Status: **fixed** in `cce7b1c` (second UH pass, 20260825) via the "tighten
the content discriminator" option below, option 2. See "The fix that
shipped" at the end of this note for what landed, the real-byte negative
control that reproduces the exact regression the first attempt caused, and
the full seven-corpus verification. The rest of this note (through "What a
safe fix needs") is preserved as originally written, describing the state
before this fix and the reverted first attempt.

## The defect

`unpack_module_blob_text` (`src/module_blob.rs`) only understands one wire
shape for a module body: a Format15 ("V8 container") blob, opened by the
`0x7FFFFFFF` sentinel, holding a named `"text"` element. Some module bodies
carry no such framing -- the row's own raw-deflate-inflated bytes *are* the
final module text already, BOM-prefixed exactly like the platform's own
`Ext/Module.bsl`. `unpack_module_blob_text` fails closed on these
(`UnexpectedFileMarker`, since the first four bytes are the UTF-8 BOM, not
the sentinel), and none of the existing fallbacks in
`dump_row` (`form_body_module_text_bytes`, `unpack_form_body_module_text`)
recognize a plain-text body either, so the row ends up `opaque` with `no
legacy family decoder recognized this storage entry`.

Confirmed on three real ERP УХ 3.2.12.6 `CommonModules` (`1cv8.cf`, `cf
extract <uuid>.0`, inflated bytes diffed byte-for-byte against
`$D/cap/uh-r1/src/CommonModules/<name>/Ext/Module.bsl`, `diff` exit 0 on
all three):

| module | uuid | plaintext bytes |
| --- | --- | ---: |
| БПМСФОУХ | `5b02973d-ada1-48eb-bf0a-f039f18b269d` | 4,176 |
| ВариантыОтчетовПереопределяемый | `27651e0e-d560-434b-a49c-f473a5b20c4d` | (matched) |
| ВзаиморасчетыВызовСервера | `3004a91e-0ecc-4c8b-9e3d-b901f848cff9` | (matched) |

On the full ERP УХ corpus this is a genuinely separate defect from the
header-wrapper arity bug fixed in `ab58c3f`: fixing that bug (which closes
the object's own *descriptor*) drops `CommonModules` from the opaque bucket
from 232 files to 73 -- the 159-file overlap is the arity bug; these 73 are
this module-body-framing gap, previously invisible because the descriptor
failure hid the whole object before the arity fix landed. `Catalogs` and
`Reports`'s own remaining opaque buckets (119 and 87 files, unchanged by
the arity fix) are a *mix* -- some entries are the object's own descriptor
(a still-unidentified, separate cause), others are exactly this
module-body shape (`Ext/ManagerModule.bsl`, `Ext/ObjectModule.bsl`).

## The fix that was tried, and why it was reverted

Adding a permissive fallback -- "if the inflated bytes are BOM-prefixed and
valid UTF-8, treat them as the module text" -- gated only on
`context.module_text_paths.contains_key(file_name)` (the same gate
`unpack_form_body_module_text`'s existing fallback uses) passed all three
CommonModules samples and every regression check on `ws`/`mdm`/`wms`
(`BROKEN=0` against `$D/base789` on all three), but on `sslbase` and `ssl`
it turned `extra: 0` into `extra: 120` / `extra: 126` respectively --
`exact`/`differing`/`missing` stayed exactly at baseline, but real new
`Bots/<name>/Ext/Module.bsl` files appeared that the platform never writes
(`$D/cap/sslbase/src` has no `Bots/` directory at all).

Traced to one storage row producing two outputs at once:

```
{"logical_name": "5a971dc0-28bf-4426-9426-9f456aea080a.1", "disposition": "supported",
 "outputs": ["Bots/АрхивАнкет/Ext/Module.bsl",
             "DataProcessors/ДоступныеАнкеты/Forms/АрхивАнкет/Ext/Help.xml"]}
```

The row is a form's Help topic (XML/HTML help text -- also BOM-prefixed,
also valid UTF-8, by the same convention every 1C text blob uses) that
`context.module_text_paths` *also* has a — apparently incorrect — mapping
for, associating this exact `<uuid>.<suffix>` with a `Bot` module route.
Before this change that stray mapping was harmless: neither
`unpack_module_blob_text` (wrong container shape) nor
`unpack_form_body_module_text` (wrong blob shape entirely) would accidentally
succeed on help-topic content, so the row only ever produced its one correct
source-asset output. The permissive BOM-and-valid-UTF8 test is not a safe
enough discriminator on its own -- it is true of nearly every 1C text blob
this project reads, module or not -- and `module_text_paths` turns out to
carry more candidate mappings than are actually real modules.

## What a safe fix needs

Two independent gaps, either of which would close this without the
regression:

1. **Find and fix the root collision in `module_text_paths` construction**
   (`module_body_paths_from_texts` / `parse_module_body_source_paths_from_metadata_text`
   in `mssql_dump::mod`) so a row that is genuinely a source asset (a form's
   Help topic, in the sample above) never also gets offered as a candidate
   module-body suffix for an unrelated object. This is the more correct fix
   -- it removes the false candidate instead of merely refusing to act on
   it -- but needs its own investigation into why `Bot`'s module suffix
   registration collides with this specific uuid+suffix pair.
2. **Or, tighten the content discriminator** past "BOM + valid UTF-8" to
   something that only matches real BSL source -- e.g. requiring the text
   to parse as a plausible module (leading `#Область`/`Процедура`/`Функция`/
   comment tokens, or reuse of whatever grammar `unpack_form_body_module_text`
   already trusts) rather than accepting arbitrary well-formed text. Not
   attempted here: the corpus doesn't yet have enough negative examples
   (real non-module BOM-text blobs reachable through this exact gate) to
   evidence which markers are safe to require.

Either needs verification against the same full sweep this pass used
(`ws`/`mdm`/`wms`/`sslbase`/`ssl`/`ut`/`uh`, `BROKEN=0` by exact-set diff on
all seven, not just the corpus the fix targets) before it should land --
`sslbase`/`ssl` are exactly the corpora `CommonModules`/`mdm`/`wms` did not
exercise, which is why the first attempt's regression was invisible until
those two ran.

## Reproduction

```
ibcmd-rs cf extract 1cv8.cf <uuid>.0 <outdir>          # module-body sample
diff <outdir>/unpacked.bin <native>/.../Ext/Module.bsl # confirms plain-text shape
```

No code in this commit implements or depends on the reverted fallback.

## The fix that shipped (`cce7b1c`)

Option 2 from "What a safe fix needs" above: tighten the content
discriminator past "BOM + valid UTF-8", without touching `module_text_paths`
construction (option 1, the more correct but form-classification-adjacent
fix, is still open -- see the root-cause map's "What is still open").

New function `unpack_plain_text_module_body` (`src/module_blob.rs`, next to
`unpack_module_blob_text`): inflates the raw blob itself (mirroring what
`unpack_module_blob_text` does internally before failing), requires the
inflated bytes to be BOM-prefixed and valid UTF-8, then rejects the payload
if its first non-whitespace byte after the BOM is `<`, `{` or `[`. Wired into
`dump_row`'s existing fallback chain (`mssql_dump::mod`) as a third tier,
tried only after `unpack_form_body_module_text` (the existing, safe,
structurally-typed form-body decoder) also fails:

```rust
Err(_) if context.module_text_paths.contains_key(file_name) => {
    unpack_form_body_module_text(&bytes)
        .or_else(|| unpack_plain_text_module_body(&bytes))
}
```

Why `<`/`{`/`[` and not a positive BSL-keyword allowlist: legal BSL module
bodies can open with many different constructs (a `//` comment, a `#`
preprocessor region, an annotation, a label, a bare statement), so requiring
one specific shape would fail closed on real modules the corpus has not
happened to sample yet. The three excluded bytes are, by contrast, exactly
the first byte of every non-module wrapper shape this defect's own evidence
turned up, and none of them is legal as the first token of a BSL module (not
even a `<` comparison, which needs a preceding operand).

**Negative control for the exact reverted regression.** Re-extracted the
precise colliding row named above (`sslbase`, storage element
`5a971dc0-28bf-4426-9426-9f456aea080a.1`) with `cf extract` against this
session's built binary. Two things the first attempt's writeup did not have
byte-level confirmation of, now confirmed:

- The row's raw-deflate-inflated bytes are *not* `Help.xml`'s text
  directly -- they are 1C's own typed-value wrapper around it,
  `{5,1,"ru",{#base64:...},0}` (a per-language content record; the
  base64 payload decodes to the `ru.html` help body). It opens with `{`,
  one of the three excluded bytes, independent of the `<` check that
  excludes plain XML wrappers elsewhere in the corpus.
- `unpack_module_blob_text` still rejects it (no V8 container), and
  `unpack_plain_text_module_body` now also rejects it -- reproducing the
  exact shape that caused the first attempt's regression and confirming
  this fix does not repeat it. Captured as a real-byte fixture
  (`tests/fixtures/native-evidence/8.3.27.2214/plain-text-module-body/
  form-help-topic-braced-record.bin.b64`) with a dedicated regression test
  (`module_blob::tests::
  rejects_form_help_topic_braced_record_that_caused_the_reverted_regression`).

Two more real-byte fixtures cover the positive side: `CommonModules/
БПМСФОУХ.0` and `CommonModules/ВзаиморасчетыВызовСервера.0` from ERP УХ's
own `1cv8.cf` (the latter opens with `#Область` after a leading CRLF, not a
`//` comment, exercising both confirmed leading shapes) --
`module_blob::tests::unpacks_plain_text_module_body_real_common_modules`.
Plus a synthetic-bytes test for the three-way `<`/`{`/`[` exclusion and the
BOM-required gate (`module_blob::tests::
plain_text_module_body_rejects_xml_and_json_leads_accepts_bare_statement`).

**Full seven-corpus verification**, exact-set diff against
`$D/base789/<key>.parity.json` (not counts):

```
ws        BROKEN=0  gained=0    new_extra=0
mdm       BROKEN=0  gained=0    new_extra=0
wms       BROKEN=0  gained=0    new_extra=0
sslbase   BROKEN=0  gained=0    new_extra=0   (the corpus the reverted fix broke)
ssl       BROKEN=0  gained=0    new_extra=0   (ditto)
ut        BROKEN=0  gained=0    new_extra=0
uh        BROKEN=0  gained=150  new_extra=0   (extra stays at exactly 64, unchanged)
```

`uh`'s 150-file gain by family (comparing `missing` sets before/after):
`CommonModules` 73 (fully closed, matching this note's earlier prediction),
`Documents` 44, `Catalogs` 19, `DataProcessors` 5, `InformationRegisters` 4,
`Constants` 2, `ChartsOfCharacteristicTypes` 2, `Ext` 1 -- confirming the
`Ext/ManagerModule.bsl`/`Ext/ObjectModule.bsl` portion of `Catalogs`'/
`Documents`' own remaining opaque buckets the root-cause map flagged as
"exactly this module-body shape" was in fact this same defect, just spread
across families beyond the three (`CommonModules`/`Catalogs`/`Reports`) the
map had confirmed it in directly. `uh` missing: 1,513 -> 1,363.

`cargo test --lib`: 2235/33 (was 2232/33, +3 new tests), 33 failures still
name-for-name identical to `$D/fail-base.txt`. `bundled9.sh`: 9/9. `cargo fmt
--check` and `git diff --check` clean.
