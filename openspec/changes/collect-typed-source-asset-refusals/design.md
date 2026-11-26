# Typed source-asset refusal collection

## Recoverable boundary

A refusal is collectible only when the codec classified it on the source side:

| class | collected | reason |
|---|---|---|
| `unsupported` | yes | the bytes are outside the evidenced cohort |
| `malformed` | yes | the stored body does not match its declared shape |
| `unresolved` | yes | a reference in the body could not be resolved |
| `ambiguous` | yes | the source does not determine one output |
| `invariant` | no | this tool broke its own contract |
| unclassified | no | nothing proves the failure belongs to the source |

The same rule already governs form rejections, which are admitted only with a
non-empty structured diagnostic. This change states it once and applies it to
codecs that expose `code()` and `class()`, starting with
`DcsTemplateNormalizeError`.

## Report shape

`WrittenSourceAsset` gains a typed-rejection variant carrying the family token,
the stable code, the classification and the raw length and SHA-256 of the
refused body. The caller turns it into one `SourceAssetCompletenessEntry`,
because the codec classified the whole body rather than one property slot, and
records it through the existing opaque path. Schema, clustering and the strict
gate are untouched; `family` is `dcs` for data-composition templates.

No payload bytes, decoded fragments or formatted error text enter the report.

## Non-goals

Emitting a partial `Template.xml`, weakening the release gate, or changing the
default fail-fast export. This change only makes one diagnostic run enumerate
the whole backlog instead of the first item of it.

## Verification

- A refusal class table test pins which classes are collectible.
- The diagnostic mode records a typed refusal and continues with the next row.
- The default mode still aborts on the same body.
- A full ERP UH diagnostic export completes and reports its clusters.
