# WebService native layout evidence for 8.3.27.1989

Status: implemented as the `web-service-v1-crlf-utf8-bom` experimental
platform layout. The metadata codec is standalone and does not require an
installed 1C platform, EDT, JVM, or database access.

## Retained sources

- Primary XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\WebServices\RemoteControl.xml`.
- Inflated primary metadata row:
  `E:\ibcmd_lab\batch130_134_fix_full_20260718\Config_inflated\03d08c14-f814-4e12-8f96-020c36cca2bf__part0.txt`.
- Inflated row length: 565 bytes; SHA-256:
  `388555d4162577e43c174e0cc16cd11ef15f1e38b8054b86f89b47c8d82edd61`.
- Thirteen independently paired WebService XCF/native rows were inspected.
  Together they contain 143 operations and 392 parameters, all three transfer
  directions, both Nillable values, XML Schema/core/custom XDTO types, and both
  metadata-reference and namespace-string package forms.
- `WebServices\RemoteControl\Ext\Module.bsl` is a separate source asset. Its
  native `.0` row is not embedded in or interpreted by this metadata layout.

The implementation reads these retained artifacts only as evidence. It does
not start 1C or modify Config/ConfigSave. The inflated native plaintext begins
with the required UTF-8 BOM and then uses CRLF line endings.

## Exact native grammar

The primary row has exactly four fields:

```text
{1, WebServiceObject, 1, OperationCollection}
```

`WebServiceObject` has discriminator `4`, Namespace, the common metadata
header, a typed XDTOPackage-reference collection, DescriptorFileName, a
namespace-string package collection, ReuseSessions, and SessionMaxAge. The
retained corpus establishes `DontUse -> 0` and SessionMaxAge `20` for this
layout. A metadata package reference uses reference-class UUID
`157fa490-4ce9-11d4-9415-008048da11f9` and resolves to a top-level
`XDTOPackage` UUID; the XML side uses `XDTOPackage.<Name>`.

`OperationCollection` uses fixed UUID
`36186084-c23a-43bd-876c-a3a8ba1a9622`; entries follow `UUID,count` directly.
Each entry contains an exact seven-field operation object, marker `1`, and a
parameter collection. Retained operation codes are discriminator `1`,
Transactioned `0`, and Managed data-lock mode `1`.

Each parameter collection uses fixed UUID
`b78a00b2-2260-4ef5-a70c-17889cfee695`. A parameter entry contains an exact
five-field parameter object and tail `0`. Transfer direction maps reversibly as
`In -> 0`, `Out -> 1`, and `InOut -> 2`. Operation and parameter XDTO types are
exact triples `{0, Namespace, LocalName}`. Counts, UUIDs, markers, codes,
wrappers, field order, and references are checked strictly; unsupported or
ambiguous values fail closed.

## Version boundary

XCF 2.20 and 2.21 have explicit dialect codecs over this one evidenced native
metadata layout. Platform build, storage profile, XML dialect, referenced
XDTOPackage assets, and the separate module asset remain independent axes. A
future platform must select a separately evidenced layout constant.
