# Template bodies on 8.3.27

Status: bounded evidence for the standalone compiler profile
`platform-8.3.27.1989`.

The native layouts were recovered from the independently retained inflated
`Config` corpus under
`E:\ibcmd_lab\batch133_register_roots_full_20260718`. The production codecs
are pure Rust and do not invoke 1C, EDT or a JVM. They are selected by three
independent profile constants:

- `bootstrap.body.mxl.layout = moxel-v8-raw-deflate-v1`;
- `bootstrap.body.dcs.layout = dcs-schema-three-document-v1`;
- `bootstrap.body.template.layout = template-kind-dispatch-v1`.

## Supported template kinds

The typed dispatcher recognizes `AddIn`, `BinaryData`,
`DataCompositionAppearanceTemplate`, `DataCompositionSchema`,
`GraphicalSchema`, `HTMLDocument`, `SpreadsheetDocument` and `TextDocument`.
An unknown `TemplateType`, a mismatched source model or an unknown native tail
blocks bootstrap. It is never downgraded to a generic raw body.

`AddIn` and `BinaryData` preserve the source bytes exactly inside the observed
marker-`1` base64 container. `HTMLDocument` uses the marker-`5` page/file
container and preserves ordered page names, payloads, nested file names and
nested file bytes. Unsafe or duplicate names and inconsistent counts fail
closed. `TextDocument`, `GraphicalSchema` and
`DataCompositionAppearanceTemplate` are raw-DEFLATE bodies; the XML variants
are parsed with bounded depth/node limits and require their known roots and
namespaces.

## SpreadsheetDocument / MOXCEL

The strict MXL decoder requires the complete observed prefix
`MOXCEL\0 08 00 01 00 0c 00`, a UTF-8 BOM and a marker-`8` native tree. It
validates the version, bounded column count, eight-field language descriptor
and canonical trailer `2,{0,1}`. The base-free compiler produces that layout
directly from `Template.xml`; MSSQL staging no longer reads an active body to
obtain a number-format hint. The legacy exporter delegates framing and bounded
native parsing to the same codec before formatting source XML.

Semantic tests compile the same XML twice, require identical compressed bytes,
decode the native body and export its localized text and parameter cells back
to XML. Unknown headers and roots are hard errors.

## DataCompositionSchema / DCS

An observed DCS schema plaintext starts with a 24-byte little-endian header:

1. `u32` marker `0`;
2. `u32` layout version `1`;
3. `u64` byte length of the first XML document;
4. `u64` byte length of the second XML document.

Three UTF-8-BOM XML documents follow: the main `SchemaFile`, one `Settings`
document and a trailing `SchemaFile`. The third document occupies the
remaining bytes. The compiler extracts the single source `settingsVariant`
settings block into document two, including non-empty filter/settings content;
the existing exporter inserts it back into the same variant. Header lengths,
BOMs, roots, namespaces, XML balance, depth and node counts are validated
before a body is accepted.

The evidenced layout has one settings variant. Missing/multiple variants and
inline `AreaTemplate` values that require the separately indexed native area
document remain explicit blockers. Historical direct-XML DCS rows remain
readable only through the named compatibility decoder; the profile-selected
writer and strict decoder accept only the three-document layout.

## Retained evidence

Hashes below are of the inflated native row bytes, not the compressed SQL
payload.

| Template kind | Native row | Inflated SHA-256 |
| --- | --- | --- |
| DataCompositionAppearanceTemplate | `8ccba179-0e77-4a89-9b6f-c6fd703547a9.0` | `b9893f21b1649c007cd56c44d7e5d93c8423b4330719d687aca43366a755c516` |
| DataCompositionSchema | `e730ea3f-4f2a-4e21-8851-bccc51830cfe.0` | `ac225d498fec0184cf33b28a16d2001b77fb543c0247086d6ff260ba9a5224cd` |
| SpreadsheetDocument | `d05bbd02-9db3-4830-be9d-7080f3506e01.0` | `a4ebd8366c2273c014628267da3cb285a0ac207cfb066c6c3a3858483ef6feef` |
| HTMLDocument | `3002d5d8-1b7d-4252-9e5d-9a533bf03ca8.0` | `0c11680e34f7102da858b54235263b32899427aaf429a7d5f934d990d4df09df` |
| BinaryData | `dfee8189-c97f-4140-a9c2-ede06970ca7c.0` | `d7267275830993e644efec6f2b393aeeabd1c0af2059bc08cb94e019b7664c72` |
| TextDocument | `098d9444-e178-45b5-a8c3-97bd94371684.0` | `d9fe88f410a7190a54b8bd92745c73f506dbf178d2ddeca46f050cc0ea6b939e` |
| AddIn | `1ff8210b-26c5-4429-b580-7c6ec7c9b63a.0` | `f2fe6c42a97aab0d6b30298933a8e070d596b05de5167ccd22eac7db98ea4c73` |

`GraphicalSchema` is covered by a project-owned minimal semantic fixture and
the known `http://v8.1c.ru/8.3/xcf/scheme` source dialect; this retained corpus
did not contain a matching native body. That narrower provenance is kept
explicit rather than being presented as corpus evidence.

## Version and compatibility boundary

The codecs are enabled only when the platform build is `8.3.27.1989`, the
storage profile is `storage:mssql-config-configsave`, and all three constants
above select the exact evidenced layouts. The bundled 8.3.24 and 8.5.1
profiles do not inherit them. Support for another platform version therefore
requires explicit profile evidence and cannot silently reuse these bytes.

## Verification

```text
cargo test --lib compiler::bodies::
cargo test --lib template
cargo test --lib mssql::tests::prepares_evidenced_dcs_template_without_fetching_base_blob -- --exact
cargo test --lib mssql::tests::prepares_spreadsheet_template_without_fetching_base_blob -- --exact
cargo check --workspace --all-targets
cargo fmt --all -- --check
```
