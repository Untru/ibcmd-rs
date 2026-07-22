# XDTOPackage native layout evidence for 8.3.27.1989

Status: implemented as the `xdto-package-v1-crlf-no-bom` experimental
platform layout. The metadata codec is standalone and does not require an
installed 1C platform, EDT, JVM, or database access.

## Retained sources

- Primary XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\XDTOPackages\АдминистрированиеОбменаДанными_2_4_5_1.xml`.
- Inflated primary metadata row:
  `E:\ibcmd_lab\batch130_134_fix_full_20260718\Config_inflated\ac7ea771-4b10-4d43-9c0a-9cd36e4c49a4__part0.txt`.
- Inflated row length: 328 bytes; SHA-256:
  `5ba0e642866ca3f936311361298465ab1012eca98fec2d5fa77385850612d50b`.
- Separate source asset:
  `E:\ibcmd_lab\ut\ut_ibcmd\XDTOPackages\АдминистрированиеОбменаДанными_2_4_5_1\Ext\Package.bin`;
  766 bytes; SHA-256:
  `a0dc20231e00d8add2614ec1e0b02a2c5ed51c7ff80d43eb322f8cd2b3a0401c`.
- The retained snapshot contains 60 independently paired XCF/native metadata
  rows from the 407-package source corpus.

The implementation reads these retained artifacts only as test evidence. It
does not start 1C or modify Config/ConfigSave.

## Exact metadata grammar

The primary metadata row is an exact three-field root:

```text
{1, XDTOPackageObject, 0}
```

`XDTOPackageObject` has exactly three fields: discriminator `1`, the common
metadata header, and a non-empty Namespace string. Missing, unknown,
reordered, or extra fields fail closed. The Namespace is retained as typed
canonical text and cannot contain whitespace.

`Ext/Package.bin` is intentionally outside this grammar. It is a separate
opaque source asset with its own storage route and digest; the converter does
not deserialize, regenerate, or guess its payload.

## Version boundary

XCF 2.20 and 2.21 have explicit dialect codecs over this one evidenced native
metadata layout. Platform build, storage profile, XML dialect, and opaque body
asset remain independent. A future platform must select a separately
evidenced layout constant.
