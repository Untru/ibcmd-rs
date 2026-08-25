# Open lead: module bodies with no V8 container framing, 20260825

Status: evidenced but not fixed. A real, confirmed defect with a tried fix
that caused a worse regression than it closed; reverted rather than shipped.
No code change from this note is in the tree -- `unpack_module_blob_text`
and its one call site in `mssql_dump::mod::dump_row` are exactly as they
were on 789b1ae.

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
