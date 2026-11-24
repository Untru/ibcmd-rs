# CF load over an applied configuration with a deleted common module

On 2026-11-24, the Windows executable from upstream master `54b796f6` loaded a replacement CF into six disposable databases. Each native platform exported the pending configuration, applied it, and exported it again. Both exports matched the source database across every relative path and every file byte, including `ConfigDumpInfo.xml`. The target initially contained five files. The replacement contained three.

| Native platform | File database | PostgreSQL 17.2 (1C build 17.2-4.1C) | SQL Server 17.0.1155.3 |
|---|---|---|---|
| 8.3.27.2342 | 3/3 pending and applied | 3/3 pending and applied | 3/3 pending and applied |
| 8.5.1.1529 | 3/3 pending and applied | 3/3 pending and applied | 3/3 pending and applied |

The measurements include input CF hashes and file inventories for the native exports. The tested executable came from the Windows VSIX in the upstream build. [Measurement manifest](cf-config-load-removal.json) · [Windows build](https://github.com/Untru/ibcmd-rs/actions/runs/41428030979)

## Reproduce the replacement

1. Prepare two source trees with the same configuration UUID. For 8.5 use `tests/fixtures/file-infobase/tiny-source` as the old tree. In the new tree remove `CommonModules` and its `<CommonModule>` entry in `Configuration.xml`. For 8.3 use `empty-xml-8.3.27` from the same directory as the new tree, omitting `ConfigDumpInfo.xml` on import. For the old tree copy in the common module and its BSL file from `tiny-source`, set the module XML format to `2.20`, and add its name to `ChildObjects`.
2. Use the matching native platform to create disposable file databases with `infobase create --db-path=<dir> --create-database --import=<tree> --apply --force`. Save each applied source with native `infobase config save` to obtain `old.cf` and `new.cf`. Run native `config export` on the new source database for the reference tree.
3. On each backend, use native `infobase create --create-database`, `config load old.cf`, and `config apply --force`. Export the target. Check that it contains both `CommonModules/ФайловаяБазаПроверка.xml` and `CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl`. Save the applied configuration with native `config save --db`.
4. Run **ibcmd-rs** `infobase config load <connection> new.cf`. Repeat native `config save --db`. For each saved CF, use `ibcmd-rs cf inspect` to compare the map from element name to `packed_sha256`. All seven applied elements matched their pre-load values in each target.
5. Export the pending configuration with native `infobase config export`. Run native `infobase config apply --force` and export again into another fresh directory. Compare the complete relative-path-to-SHA-256 maps against the native reference. Both common module paths must be absent.

Use the same connection arguments for each operation on a target. The file connection used `--db-path`, and the server connections used `--dbms`, `--db-server`, and `--db-name` with Windows authentication on SQL Server and `--db-user` on PostgreSQL. Each target had a separate native `--data` directory.

## What ConfigSave contained

On both SQL engines, a read of `FileName` and `DataSize` from `ConfigSave` immediately after load returned six rows. Five corresponded to the replacement CF, and the sixth was `deleted` with a stored size of 53 bytes. For each file database, loading the same CF again on a copy of the pre-load backup and running native `config save` before apply exposed the deletion list. Extracting `deleted` produced a UTF-8 BOM followed by the two removed storage names and their part numbers.

```text
2,"6d1c2a0e-1cd0-4f00-9a00-000000000001",0,"6d1c2a0e-1cd0-4f00-9a00-000000000001.0",0
```

The earlier save acceptance starts with a CF written by ibcmd-rs. This measurement exercises the opposite direction, starting with a native CF and using ibcmd-rs for the target write. The PostgreSQL executable predates the separate COPY change. [Save acceptance in #352](https://github.com/Untru/ibcmd-rs/issues/352) · [PostgreSQL COPY in #752](https://github.com/Untru/ibcmd-rs/pull/752)
