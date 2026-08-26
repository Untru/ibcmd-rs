# Form body container revision, 20260826

Base `911e86e`. Corpus: all eight stand configurations
(`$D = /Users/untru/Documents/ChatGPT/ibcmd-stand`).

## The refusal this closes

`cf export` of ERP УХ 3.2.12.6 refused 102 managed form bodies with

```
failed to parse form body from source asset <файл>:
Form body does not start with type marker 4
```

`FormBodyContainer::parse` compared the body's first slot against the string
`"4"` and refused everything else. That slot is not a magic number to match:
it is the revision the record declares for its own container.

## Census of the declared revision

Every form body of all eight corpora, read from the `.cf` and inflated,
tabulated by declared container revision (slot 0), layout revision (slot 1's
own first slot), and top-level container arity:

| corpus | forms | rev 4 / layout 50 | rev 4 / layout 49 | rev 3 / layout 49 | non-UTF-8 |
|---|---:|---:|---:|---:|---:|
| `ws` | 0 | 0 | 0 | 0 | 0 |
| `wms` | 5 | 5 | 0 | 0 | 0 |
| `mdm` | 12 | 7 | 5 | 0 | 0 |
| `sslbase` | 909 | 909 | 0 | 0 | 0 |
| `ssl` | 1 163 | 1 163 | 0 | 0 | 0 |
| `do` | 2 350 | 2 350 | 0 | 0 | 0 |
| `ut` | 5 201 | 5 201 | 0 | 0 | 0 |
| `uh` | 13 006 | 11 350 | 1 545 | 102 | 9 |

Facts this establishes:

* Exactly two container revisions occur: `4` everywhere, `3` on 102 managed
  ERP УХ forms and nowhere else. No third value exists in 22 646 forms.
* **Both revisions carry the identical ten-slot container.** Every managed
  body in every corpus has exactly 10 top-level fields, and the leading token
  of each is the same in both revisions: slot 0 the revision, slot 1 the
  layout block, slot 2 the quoted module text, slots 3..7 five blocks, slots
  8 and 9 two scalars. Not one revision-3 body deviates.
* The container revision is not the layout revision. Layout revision `49`
  occurs under both container revisions; layout `50` only under container `4`.
  Layout `49` was already fully read -- in `uh` alone, 705 of its bodies were
  byte-exact at `911e86e` and another 819 were `differing`, i.e. written.

The nine non-UTF-8 bodies are the ordinary forms; see
`uh-ordinary-form-body-20260826.md`.

Sample pair, revision 3 (`InformationRegisters/ФайлыСведенийРОКИ/Forms/
ФормаСписка`) against revision 4 (`Catalogs/РазделыСверкиВГО/Forms/
НастройкиЗаполнения`), both layout 49:

```
rev 3: {3, {49,0,0,741,0,1,...}, "<module>", {4,1,...}, {0,0}, {0,1,...},
          {0,0}, {0,0}, 0, 0}
rev 4: {4, {49,0,0,0,0,1,...},   "<module>", {4,6,...}, {0,1,...}, {0,1,...},
          {0,0}, {0,0}, 0, 0}
```

## The fix

`FormBodyRevision::parse` (`src/module_blob.rs`) reads slot 0 and names the
revision; `ParsedFormBodyBlob` carries it. Both admitted revisions share the
one container reader, because the census says they share the container. An
undeclared or unnamed revision is still refused, and the refusal now names
what the record declared:

```
Form body declares unsupported container revision <value>
Form body declares no container revision
```

`module_blob::tests::rejects_non_form_body_blob` codified the old white-list
message and was rewritten against the new one (doctrine 8);
`parses_revision_three_form_body_container` pins the new dispatch.

## Result

`uh`: all 102 revision-3 bodies are now read. 34 of their `Ext/Form.xml`
files came out byte-exact against the platform on the first pass, 68 came out
`differing` -- ordinary parity work of the same kind as the corpus's other
1 381 `differing` managed forms, not a decode failure. `BROKEN = 0` and
`extra = 0` on all eight corpora.

## Still open

The 68 `differing` revision-3 forms have not been diffed field by field yet.
They are now visible to the normal `differing` workflow, which they were not
while the container reader refused them.
