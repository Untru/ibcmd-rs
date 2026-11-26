# IntegrationService native layout evidence for 8.3.27.1989

Status: implemented as the `integration-service-v1-crlf-utf8-bom`
experimental platform layout. The codec is standalone and does not require an
installed 1C platform, EDT, JVM, or database access.

## Retained sources

- XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\IntegrationServices\ОбменСообщениями.xml`;
  5,265 bytes; SHA-256
  `e8fd990d462e269752a28e35778d4c6fcac4977a8cb43502a12e4d1ce0c7a731`.
- The source contains four independently identified channels: two `Receive`
  channels with handlers and `Transactioned=false`, and two `Send` channels
  without handlers and with `Transactioned=true`.
- The exact one-channel native metadata fixture is retained by
  `mssql_dump::tests::extracts_integration_service_channels_to_metadata_xml`.
  The compiler golden test matches that plaintext byte-for-byte after the
  profile-mandated UTF-8 BOM.
- `IntegrationServices\ОбменСообщениями\Ext\Module.bsl` remains a separate
  source asset and is not embedded in the primary metadata row.

The implementation uses these artifacts only as offline evidence. It does not
start 1C or modify `Config`/`ConfigSave`. The selected profile emits a UTF-8
BOM and deterministic CRLF text before raw DEFLATE.

## Exact native grammar

The primary row has exactly four fields:

```text
{1, IntegrationServiceObject, 1, ChannelCollection}
```

`IntegrationServiceObject` has exactly five fields: discriminator `0`, the
common metadata header, Manager TypeId, Manager ValueId, and external service
address.

`ChannelCollection` has fixed UUID
`acb7e81f-0637-4ebd-88ff-954ba075ae51`, an exact count, and one wrapper list.
Each wrapper contains exactly a channel object and tail `0`. A channel object
has exactly eight fields: discriminator `1`, common metadata header, Manager
TypeId, Manager ValueId, external channel name, receive handler, direction
code, and transaction flag. Direction maps reversibly as `Send -> 0` and
`Receive -> 1`.

For this evidenced layout, a receive channel requires a non-empty identifier
handler and `Transactioned=false`; a send channel requires an empty handler
and `Transactioned=true`. Root, channel, generated TypeId, and generated
ValueId identities are non-nil and globally distinct. Unknown collection
UUIDs, counts, wrappers, fields, codes, identity aliases, or direction-specific
property combinations fail closed.

## Version boundary

XCF 2.20 and 2.21 have explicit dialect codecs over this one evidenced native
layout. Platform build, storage profile, XML dialect, and the separate module
asset remain independent axes. A future platform must select a separately
evidenced layout constant.
