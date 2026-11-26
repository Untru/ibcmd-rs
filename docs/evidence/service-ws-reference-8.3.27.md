# WSReference native layout evidence for 8.3.27.1989

Status: implemented as the `ws-reference-v1-crlf-utf8-bom` experimental
platform layout. The metadata codec is standalone and does not require an
installed 1C platform, EDT, JVM, or database access.

## Retained sources

- XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\WSReferences\UpdateFilesApiImplService.xml`;
  1,604 bytes; SHA-256
  `f1aa8a179d0a1563394ee10c2457d2d70aeaf9a4c6ec56ece2deb81595dd361c`.
- Exact native metadata fixture retained by
  `mssql_dump::tests::extracts_sfc_metadata_family_xml_from_blob_shapes` and
  `mssql_dump::tests::writes_ws_reference_body_asset_to_source_layout`.
- Separate WSDL source asset:
  `E:\ibcmd_lab\ut\ut_ibcmd\WSReferences\UpdateFilesApiImplService\Ext\WSDefinition.xml`;
  10,294 bytes; SHA-256
  `931d54e0a6e1daefd18813935744314d223824445f6f14b36f4c771c7aba386a`.

The compiler consumes no installed-platform API. The profile applies the
platform-wide evidenced UTF-8 BOM plus CRLF plaintext contract before raw
DEFLATE.

## Exact native grammar

The primary metadata row is an exact three-field root:

```text
{1, WSReferenceObject, 0}
```

`WSReferenceObject` has exactly five fields: discriminator `2`, a two-field
LocationURL wrapper whose tail is `0`, the common metadata header, Manager
TypeId, and Manager ValueId. The generated identities are non-nil, distinct,
typed as category `Manager`, and reconstructed in XCF as
`WSReferenceManager.<Name>`. Unknown wrappers, tails, reordered fields, extra
fields, malformed URLs, and duplicate identities fail closed.

`Ext/WSDefinition.xml` is intentionally a separate opaque source asset and is
not embedded in this metadata row.

## Version boundary

XCF 2.20 and 2.21 have explicit dialect codecs over this one native layout.
Platform build, storage profile, XML dialect, and the WSDL asset remain
independent axes. A future platform must select a separately evidenced layout
constant.
