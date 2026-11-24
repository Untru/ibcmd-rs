# SQL Server and the BSP 3.1.7.343 demonstration configuration

Refs #361. The Windows lab used a copy of the vendor BSP 3.1.7.343 demonstration configuration, `1Cv8_demo.cf` (96,467,423 bytes, SHA-256 `8dc96b4c0f0f3537ccde4379fc77346b185375782d867cd597b19e7be401971e`). The copy matched the distribution by SHA-256. Each native build initialized a new SQL Server database with Russian locale and Windows authentication, loaded that same CF and applied it. Each source held 8,838 rows in `Config` and zero in `ConfigSave` after apply.

| Native version reported by ibcmd | Authenticode | ibcmd.exe SHA-256 | Native create / load / apply |
| --- | --- | --- | --- |
| 8.3.24.1819 | Valid, LLC 1C-Soft | `028d220cf2faaeccfcc940af89f84c9c8d3db7aadf1bd2b6429419279f468343` | 0 / 0 / 0 |
| 8.3.26.1656 | Valid, LLC 1C-Soft | `cd23866d4110fd21e0194677adbc2e1e7ca67afd18ec4f3ee4437dd73a76fb7c` | 0 / 0 / 0 |

The 8.3.24.1819 administrative image and the installed executable had identical hashes. The candidate was built from `54b796f64da5daed3714acdf7810581fe0e58778` with Rust 1.95.0 and `cargo build --locked --profile iter --features platform-oracle`. Its SHA-256 was `031a2f0ce703e84fd94185bc024eda7951f1f129245b97db14a76edbe39ddb71`.

## Observed export boundary

On 8.3.24.1819 the native XML export returned 0 and the root metadata declared version `2.17`. The same source passed to the candidate with `--platform=8.3.24.1819` returned 2 at argument parsing. The experimental seed profile for this build declares no XML dialect. The drop-in command lists the 8.3.27 and 8.5.1 profiles. With the platform option omitted, the candidate reached the SQL connection and returned a TLS `UnknownIssuer` error on this host. That invocation produced no comparable export.

## Native target results

The 8.3.24.1819 native import into an empty, independently initialized SQL target returned -1. It reported two unresolved references to the same predefined chart-of-characteristic-types element. On a second empty target the exact-build candidate import returned 2. Omitting the platform option made the candidate read the XML root and refuse dialect 2.17 before writing. Both content fingerprints of the candidate target remained at their initial empty values.

The 8.3.26.1656 native source export returned 0. It contained 10,983 files and 236,092,971 bytes of XML and ancillary files. Its XML root declared format 2.19. The candidate exact-build export returned 2. Native import of that native tree into a second SQL database returned 0 and staged 8,360 rows in `ConfigSave`. Immediate native reexport returned -1 with a configuration structure integrity error. At this step `Config` held zero rows.

The failure diagnostics above identify metadata, dialect selection and SQL transport boundaries. The examined commands reported no licensing errors.

## Reproduction

Create an empty disposable SQL database with collation `Cyrillic_General_CI_AS`. Run the following with the exact native executable. Supply `--dbms=MSSQLServer --db-server=<server> --db-name=<owned-database> --data=<private-work-dir>` after each verb.

```text
ibcmd infobase create <connection-options> --locale=ru_RU
ibcmd infobase config load <connection-options> <demo.cf>
ibcmd infobase config apply <connection-options> --force
ibcmd infobase config export <connection-options> <fresh-native-tree>
```

Compare both implementations on the same frozen source. For the import comparison, initialize two further empty disposable databases with the same native build, import the native tree into each target using its assigned implementation, and reexport each target with native ibcmd. Keep private SQL and configuration contents in the lab.
