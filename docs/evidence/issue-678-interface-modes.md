# Interface compatibility modes on 8.5.1.1529

The platform executable used for the measurement has SHA-256
`4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20`.
The public corpus is pinned at `8d138b896f462c3053d16076cbabf0ffe5030c1c`.

## Reproduce the small configuration

Start from the native XML of `8.5.1.1529/corpus-hybrid-interface`. Keep its
language and the two common forms `CovHybridModeTaxi` and
`CovHybridModeVersion85`, including their bodies. In the configuration's
child list, keep the language and these two forms. Append the data processor
`CovModernOwnedProcessor` from `corpus-modern-owned-interface` with only
its `ModeTaxi` and `ModeVersion85` child declarations, descriptors and bodies.
Keep the hybrid configuration's original `TaxiEnableVersion8_5` property.

Use disposable directories throughout. Create two empty file infobases on
8.5.1.1529 with locale `ru_RU`. For each native command pass a private
`--data` directory. On the first database run native `infobase config import`,
`config save` and `config export`. Use the exported XML as the candidate
input and byte-comparison oracle.

| Native XML value | Stored record | Zero-based field(s) | Value |
| --- | --- | --- | --- |
| Taxi | Form revision 14, common and owned | 5 | 1 |
| Version85 | Form revision 14, common and owned | 5 | 2 |
| TaxiEnableVersion8_5 | Configuration V76 | 38, 62 | 3, 4 |

These values were read directly from the inflated primary CF entries,
matching form names and descriptor UUIDs to the native XML. The fresh
native CF hash was `5b65fa497f8f4b0ef3dce2f085695c7454066373ba1bdf52250c7c38a8e6778f`.
The saved CFs of the hybrid, modern form, modern owned form, navigation
and combined corpus sets corroborate the relevant values in each set.

## Check each writer and reader

On the clean base `54b796f6` the CF and file-infobase writers each reported
five descriptor refusals on this input. With the fix, run:

```text
ibcmd-rs cf bootstrap --base-free --platform 8.5.1 NATIVE_XML candidate.cf
ibcmd-rs cf export candidate.cf CF_XML --source-version 2.21
ibcmd-rs infobase config import --db-path=EMPTY_DB NATIVE_XML
ibcmd-rs infobase config export --db-path=EMPTY_DB DB_XML
ibcmd infobase create --db-path=NEW_DB --create-database --locale=ru_RU --load=candidate.cf --apply --force
ibcmd infobase config export --db-path=NEW_DB NATIVE_CF_XML
ibcmd infobase config apply --db-path=EMPTY_DB --force
ibcmd infobase config export --db-path=EMPTY_DB NATIVE_DB_XML
```

All four output trees contained the same 11 application files as the native
export, byte for byte. In the twelfth file, `ConfigDumpInfo.xml`, only the
11 generated `configVersion` attributes differed. The candidate CF carried
the same four form codes and the same root pair as the native CF. The
application-tree digest was
`17f03547517e1923f6d859b019f4d51a136802f8814f1d7c0ab3ad81c9483cf5`
(SHA-256 of the concatenated, sorted UTF-8 relative paths, each followed by
NUL and the 32-byte SHA-256 of its file).

## Combined corpus boundaries

On master the mobile permission message is the first root refusal for
combined 8.5.1.1529. PR #699 addresses that record. On a scratch copy
with just the one `app:permissionMessage` element removed, both candidate
writers reached the panel height in `Ext/ClientApplicationInterface.xml`.
PR #722 covers this separate record. The remaining #678 cases concern help
links and standard commands.
