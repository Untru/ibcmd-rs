# HTTPService native layout evidence for 8.3.27.1989

Status: implemented as the `http-service-v1-crlf-utf8-bom` experimental
platform layout. The metadata codec is standalone and does not require an
installed 1C platform, EDT, JVM, or database access.

## Retained sources

- Primary XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\HTTPServices\Биллинг.xml`.
- Inflated primary metadata row:
  `E:\ibcmd_lab\batch130_134_fix_full_20260718\Config_inflated\db821e7a-ff22-4889-b166-1a1bc1118587__part0.txt`.
- Inflated row length: 2,853 bytes; SHA-256:
  `43fdb3628fbaf75c313f8dc1c8cb5ca3bd98c87dcac59624e0875aa47c1202ee`.
- The retained snapshot contains three independently paired XCF/native
  HTTPService metadata rows. Together they demonstrate nested URLTemplate and
  Method collections, every retained `ReuseSessions` code, and the `DELETE`,
  `GET`, `POST`, and `PUT` method codes.
- `HTTPServices\Биллинг\Ext\Module.bsl` is a separate source asset. Its native
  `.0` row is not embedded in or interpreted by this metadata layout.

The implementation reads these retained artifacts only as evidence. It does
not start 1C or modify Config/ConfigSave.

The inflated native plaintext begins with the required UTF-8 BOM and then uses
CRLF line endings.

## Exact native grammar

The primary metadata row has exactly four fields:

```text
{1, HTTPServiceObject, 1, URLTemplateCollection}
```

`HTTPServiceObject` has exactly five fields: discriminator `2`, RootURL text,
the common metadata header, a `ReuseSessions` code, and SessionMaxAge. The
reversible session mapping is `DontUse -> 0`, `Use -> 1`, and `AutoUse -> 2`.

`URLTemplateCollection` has the fixed UUID
`ec6896c2-9b28-42d8-9140-48491146b8ea`, an exact count, and a list. Each entry
contains a three-field URLTemplate object, marker `1`, and its method
collection. The URLTemplate object is discriminator `0`, Template text, and a
common metadata header. Entries follow `UUID,count` directly; there is no
additional collection wrapper.

Each method collection has fixed UUID
`21c96ea8-c8fc-424a-a0b4-e1ffb2fa1a73`, an exact count, and a list. A method
entry contains an exact four-field object plus tail marker `0`: discriminator
`0`, Handler text, method code, and common metadata header. Retained reversible
method mappings are `DELETE -> 2`, `GET -> 3`, `POST -> 11`, and `PUT -> 14`.
Method entries likewise follow `UUID,count` directly. Unknown codes, UUIDs,
counts, markers, reordered fields, extra wrappers, and extra fields fail closed.

## Version boundary

XCF 2.20 and 2.21 have explicit dialect codecs over this one evidenced native
metadata layout. Platform build, storage profile, XML dialect, and the separate
module asset remain independent axes. A future platform must select a
separately evidenced layout constant.
