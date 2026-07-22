# Modules, commands and source assets on 8.3.27

Status: bounded evidence for the standalone compiler profile
`platform-8.3.27.1989`.

The metadata layouts were recovered by comparing XCF sources with inflated
`Config` rows retained under
`E:\ibcmd_lab\batch133_register_roots_full_20260718`. The implementation is
pure Rust and does not invoke 1C, EDT or a JVM at runtime.

## Verified metadata rows

| Family | Representative UUID | Native shape |
| --- | --- | --- |
| CommonModule | `db63155a-3df1-44a7-aabc-dd906b877c3f` | root `1`, object marker `12`, exact metadata header and eight module flags |
| CommonCommand | `129f704b-cc63-4a4e-b6a7-9e53de4537a2` | root `1`, command collection marker `2`, fixed command ValueId and exact 13-field details |
| CommandGroup | `ac39b903-0c60-417e-a50f-49ed375424f5` | root `1`, object marker `3`, picture/category/representation/tooltip/header |
| CommonPicture | `42e39ef7-268f-4053-b4e5-4bceab17d3e4` | root `1`, object marker `4`, header and two availability flags |

All rows use raw DEFLATE over UTF-8 text with a BOM. The serializer retains
the evidenced CRLF placement, including selective line breaks and the final
line break inside the CommonCommand collection. The independent decoder
checks list arity, markers, field types and tails and rejects unknown layouts.

The XCF layer accepts dialects `2.20` and `2.21` and projects only the exact
typed property inventories required by these four families. CommonCommand and
CommandGroup picture references, command groups and DefinedType parameters are
resolved through the validated canonical graph; missing, duplicate or unknown
references fail closed. A conversion may change only the declared root XCF
version unless a future dialect migration explicitly defines semantic changes.

## Verified source-asset layouts

| Asset | Registry route | Codec |
| --- | --- | --- |
| module text | family-specific `Ext/*.bsl` suffix | V8 `text`/`info` container, raw DEFLATE |
| common/configuration picture | `Ext/Picture.xml`, `Ext/Splash.xml` or `Ext/MainSectionPicture.xml` | marker `1`, transparent-pixel tuple, strict base64 payload |
| XDTO and configuration binary | registered `Ext/*.bin` suffix | bounded raw DEFLATE |
| help | profile-selected help suffix plus `Ext/Help.xml` contributors | marker `5`, ordered named pages/files and strict base64 payloads |

`SourceAssetRegistry` is the single table for family, semantic role, native
suffix, relative source path and codec. Both standalone compilation and the
legacy MSSQL source import/export bridge consume this table. Form modules are
registered as contributors to the aggregate `ManagedForm` body codec rather
than being incorrectly emitted as independent module rows; the evidenced
marker-50 codec is documented in
[Managed Form and CommandInterface evidence](forms-command-interface-8.3.27.md).

The codecs enforce the shared compressed/uncompressed size and expansion-ratio
limits. Tests compare exact module bytes and content SHA-256 values after
module, picture, binary and help round-trips; malformed base64, unexpected
native fields and unevidenced future profile coordinates are rejected.

## Compatibility boundary

The layouts are enabled only when all independent coordinates match:

- XML dialect `2.20` or `2.21` for metadata XML;
- platform build `8.3.27.1989`;
- storage profile `storage:mssql-config-configsave`;
- no inferred compatibility mode or container revision;
- the family- or asset-specific layout constant in the platform profile.

A new platform release adds an evidence/profile entry and, only when its bytes
actually differ, a new layout implementation. Existing version-to-version
conversion behavior is therefore not silently changed.
