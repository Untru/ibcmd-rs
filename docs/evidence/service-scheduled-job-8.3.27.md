# ScheduledJob native layout evidence for 8.3.27.1989

Status: implemented as the `scheduled-job-v1-crlf-no-bom` experimental
platform layout. The codec is standalone: it does not require an installed 1C
platform, EDT, JVM, or database access.

## Retained sources

- XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\ScheduledJobs\ЗагрузкаКурсовВалют.xml`.
- Inflated primary row:
  `E:\ibcmd_lab\batch130_134_fix_full_20260718\Config_inflated\c7ffd8ab-15e9-4cf1-a7fd-d05534dff000__part0.txt`.
- Inflated row length: 317 bytes; SHA-256:
  `aaf1d537769ff9b40614065b563f1eaf3a95d2c57f6eb536702c9a982fe66eae`.

The implementation only reads retained artifacts used as test evidence. It
does not start 1C or modify Config/ConfigSave.

## Exact native grammar

The primary row is an exact three-field root:

```text
{1, ScheduledJobObject, 0}
```

`ScheduledJobObject` has exactly ten fields: discriminator `2`, common
metadata header, `Description`, `Key`, `Use`, `Predefined`, CommonModule UUID,
method name, retry count, and retry interval. The common header has its exact
nine-field shell and uses a counted language/content synonym list. Text uses
1C doubled-quote escaping. Booleans are `0` or `1`; counts are canonical
unsigned decimal values. Missing, reordered, unknown, or extra fields fail
closed.

XCF stores the handler as
`CommonModule.<module-name>.<method-name>`, while the native row stores the
module UUID and method name separately. Compilation resolves the module name
against the validated canonical graph and rejects missing or ambiguous
modules. Native IR-to-XCF requires the caller to provide the exact canonical
CommonModule reference, so the opaque UUID is never guessed.

## Version boundary

XCF 2.20 and 2.21 use explicit dialect codecs over the same evidenced native
layout. Platform build, storage profile, and XML dialect remain independent
axes. A future platform must opt into a separately evidenced layout constant;
it cannot silently inherit these bytes.
