# SQL export and import on three exact Windows builds

Refs #361. All three native builds loaded the same BSP 3.1.7.343 demonstration CF, applied it and exported XML from SQL Server. The root dialect changed with each release. The candidate refused all three exact builds on export and import. [The acceptance audit](acceptance-2026-11-24.json) retains three blocked rows.

## Binaries and corpus

The candidate was `ibcmd-rs 0.4.1` at `54b796f64da5daed3714acdf7810581fe0e58778`, built with Rust 1.95.0 and `cargo build --locked --profile iter --features platform-oracle`. Its SHA-256 was `031a2f0ce703e84fd94185bc024eda7951f1f129245b97db14a76edbe39ddb71`. Each native executable reported its exact version via `--version` and had a valid LLC 1C-Soft Authenticode signature. The administrative images used for 8.3.24 and 8.3.25 matched the corresponding installed executables by SHA-256.

| Native build | ibcmd.exe SHA-256 |
| --- | --- |
| 8.3.24.1819 | `028d220cf2faaeccfcc940af89f84c9c8d3db7aadf1bd2b6429419279f468343` |
| 8.3.25.1633 | `9c19e8355e1f7c699100be3184b31c0b206ef9c9f24c5f0974ef59c870b06e2d` |
| 8.3.26.1656 | `cd23866d4110fd21e0194677adbc2e1e7ca67afd18ec4f3ee4437dd73a76fb7c` |

The lab copy of the vendor's `1Cv8_demo.cf` was 96,467,423 bytes and matched the original by SHA-256, `8dc96b4c0f0f3537ccde4379fc77346b185375782d867cd597b19e7be401971e`. Each build used Windows authentication on SQL Server 17.0.1155.3, a separate source database and two distinct native-initialized targets. The initial `Config` and `ConfigSave` counts on every target were zero.

## Native exports

Native create, CF load, apply and full export all returned 0. File totals include every relative path under the export directory. The tree SHA-256 hashes the concatenation of each UTF-8 relative path (slash separators), a NUL byte and the raw 32-byte SHA-256 of its file, in Unicode code-point path order. Sizes are uncompressed bytes.

| Native build | Configuration.xml dialect | Files | Bytes | Tree SHA-256 |
| --- | --- | ---: | ---: | --- |
| 8.3.24.1819 | 2.17 | 10,983 | 235,904,419 | `613b5500e884cb597b6b99e62d227e3715b742b228c1848a753ef27735a3b358` |
| 8.3.25.1633 | 2.18 | 10,983 | 236,093,508 | `9f2ec0bd3e4071d734dc8e64ad31352f2437f58d4a38897187531ab21b78862e` |
| 8.3.26.1656 | 2.19 | 10,983 | 236,092,971 | `42e1f5dc88344869f28584b772528e9620452cb29030198a74624ec2d3bd29fc` |

## Imports into empty targets

The input for each pair of targets was that build's native export. A dash means that native import had already failed. The last column gives SQL row counts after the listed operations.

| Native build | Native XML import | Immediate native reexport | Native apply after reexport failure | Config / ConfigSave |
| --- | ---: | ---: | ---: | ---: |
| 8.3.24.1819 | -1 | — | — | 0 / 20 |
| 8.3.25.1633 | -1 | — | — | 0 / 3 |
| 8.3.26.1656 | 0 | -1 | -1 | 0 / 8360 |

On 8.3.24 and 8.3.25 the native importer reported two unresolved references to the same predefined chart-of-characteristic-types element. Both failures left staged rows. On 8.3.26 the native import succeeded, but both reexport and activation reported a configuration structure integrity error. Activation left the staged and active row counts unchanged.

On all three builds both drop-in commands with `--platform=<exact-build>` returned 2 at argument parsing. The 8.3.24.1819 experimental seed profile declares no XML dialect. Without the platform argument, each candidate import returned -1 and identified the native XML dialect as unsupported. Each candidate export without that argument returned -1 at SQL TLS negotiation with `UnknownIssuer` on this endpoint. The latter is a transport observation; it supplies no evidence about default export on a connection with a trusted certificate.

The source fingerprints of `Config` and `ConfigSave` matched before and after the comparisons. Fingerprints covered rows ordered by binary filename and part number, both timestamps, attributes, declared size and SHA-256 of stored bytes. The candidate targets' same two tables matched their empty fingerprints after the explicit and default import attempts. The logs contained no licensing refusal. None of these routes produced two successful output trees for a candidate parity diff.

## Layout and write scope

Each source stored 8,834 logical configuration files in 8,838 `Config` rows, with a maximum part number of 2 and an empty `ConfigSave`. The seven columns observed in both tables were `FileName`, `Creation`, `Modified`, `Attributes`, `DataSize`, `BinaryData` and `PartNo`. Stored payload bytes totaled 93,747,055 on 8.3.24 and 8.3.25 and 93,747,054 on 8.3.26. The binary layouts of individual metadata objects and form bodies remain unclassified. These results establish neither candidate write compatibility nor supported production writes on these releases.

## Reproduction

Start with an empty disposable SQL database using collation `Cyrillic_General_CI_AS`. Supply `--dbms=MSSQLServer --db-server=<server> --db-name=<database> --data=<private-work-dir>` for each database operation. Use fresh tree directories and the matching native executable for each build.

```text
ibcmd infobase create <source-options> --locale=ru_RU
ibcmd infobase config load <source-options> <demo.cf>
ibcmd infobase config apply <source-options> --force
ibcmd infobase config export <source-options> <native-tree>
ibcmd-rs infobase config export <source-options> --platform=<exact-build> <candidate-tree>
ibcmd infobase create <native-target-options> --locale=ru_RU
ibcmd infobase config import <native-target-options> <native-tree>
ibcmd infobase config export <native-target-options> <native-reexport-tree>
ibcmd infobase create <candidate-target-options> --locale=ru_RU
ibcmd-rs infobase config import <candidate-target-options> --platform=<exact-build> <native-tree>
```

After the failed native reexport on 8.3.26, the activation probe was `ibcmd infobase config apply <native-target-options> --force`. The default candidate probes omitted `--platform`, used a fresh export directory and reused the verified-empty candidate target. The read-only audit command below returns 2 (`blocked`).

```text
python scripts/platform-lab/acceptance.py check docs/evidence/platform-8.3.24-26/acceptance-2026-11-24.json --evidence-root <lab-root>
```

## Small configuration with a managed form

The follow-up on SQL Server used the repository's `tests/fixtures/external/old_roots/v59_form/input.cf` (11,091 bytes, SHA-256 `b604f9d3efa7c69cc80a5ba65ff9edfec54e1375d58d26faed8faf39ff864da7`). Each build loaded and applied the CF in its own database, then used two separately initialized targets for the XML import probes.

| Build | Native source load / apply / export | Native XML import / immediate reexport | Candidate exact-build export / import |
| --- | --- | --- | --- |
| 8.3.24.1819 | 0 / 0 / 0 | 0 / 0 | 2 / 2 |
| 8.3.25.1633 | 0 / 0 / 0 | 0 / 0 | 2 / 2 |
| 8.3.26.1656 | 0 / 0 / 0 | 0 / 0 | 2 / 2 |

Every source export contained seven files. Every immediate reexport contained three. The immediate reexport ran before applying the staged XML import. The file counts differ, so the successful command exits establish the native import boundary without establishing a round-trip match. The candidate still refuses the exact build at argument parsing.
