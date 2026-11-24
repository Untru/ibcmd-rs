# SQL Server config save acceptance, 24 November 2026

Each row compares a fresh native export of a database loaded from the
product CF with a native reference export. Both the relative file list and
every file's bytes matched, including `ConfigDumpInfo.xml`, BSL and binary
sidecars. A second check compared the element names and packed SHA-256 values
from `cf inspect` of the product and native CFs saved from the same source
state and with the same flag.

| Native platform | State at save | Matching CF elements | Matching files | Full tree SHA-256 |
|---|---|---:|---:|---|
| 8.3.27.2342 | stage | 8 | 7 | `161fa5fe9dc5546a8fbe6b50b46d2451b053daa9a96abb1b50fffd16432074c2` |
| 8.3.27.2342 | stage, `--db` | 8 | 7 | `308d60322adead3b4cad2998696a9a5d4aedbb42841cc8aa025dd6b2507f6890` |
| 8.3.27.2342 | online | 8 | 7 | `161fa5fe9dc5546a8fbe6b50b46d2451b053daa9a96abb1b50fffd16432074c2` |
| 8.3.27.2342 | multipart | 10 | 9 | `1ebdf2059feabd18e143cd71308f7a589e1f59b1f6f4492219206d4eb474a8c8` |
| 8.5.1.1529 | stage | 7 | 5 | `e07edb502c643e63861ce6f3332db34e081252ab3bfb4422d4eed7a42f889813` |
| 8.5.1.1529 | stage, `--db` | 7 | 5 | `1438415a6fd8f8f89ca244bc87f5696bacf2005d8577b68693dbff9303040060` |
| 8.5.1.1529 | online | 7 | 5 | `e07edb502c643e63861ce6f3332db34e081252ab3bfb4422d4eed7a42f889813` |
| 8.5.1.1529 | multipart | 10 | 8 | `608d52cc9e25f9f46b5077e940bf5e3232bf03f1fd48ed67199c55cb89cca27e` |

## Reproduction

SQL Server 17.0.1155.3, Standard Developer Edition, Windows integrated login.
The measured executable was built from `54b796f64da5daed3714acdf7810581fe0e58778`
using `cargo build --locked --bin ibcmd-rs`, Rust 1.98.0, debug info disabled.
Its SHA-256 was `919eaf7a3e3afbc976d199a36ee6ed78daa59f51b335ea926b0572f867453cc6`.
The local SQL instance uses a self-signed certificate. The product save used
its ODBC tools backend (`--sqlcmd`) with local launchers forwarding all
arguments to the installed clients and appending `-C` to sqlcmd and `-u` to bcp.

1. On each platform create a disposable database with native
   `infobase create --create-database --locale=ru_RU`. Supply
   `--dbms=MSSQLServer --db-server=localhost --db-name=<database>` on each
   command and a separate native `--data=<directory>` for each database.
   Load and apply `tests/fixtures/external/search_form/input.cf` on 8.3,
   or `tests/fixtures/file-infobase/tiny.cf` on 8.5.
2. Export the base with native `infobase config export <base>`. In a copy,
   append the ASCII bytes `\r\n// issue 352 staged edit\r\n` to the existing
   form module (8.3) or common module (8.5). Run native `config import <copy>`.
   SQL showed six rows in `ConfigSave` on 8.3 and five on 8.5. Save with
   and without `--db` before applying. For the default save, use the native
   export of the staged source as reference. For `--db`, use the saved base
   export. The module edit distinguished these two references.
3. Run native `config apply --force --dynamic=force`. Confirm that
   `ConfigSave` is empty and that `Config` has `DynamicallyUpdated` and
   `*_dynupdate_*` rows. Export the source again, then save its online
   configuration. The reference export still contains the module edit.
4. For multipart, add a common template of type `BinaryData` to a copy
   of an exported configuration and list it in `Configuration.xml`. Write
   `random.Random(352).randbytes(10_050_000)` (Python 3.12) to its
   `Ext/Template.bin`. Use native import and exclusive apply. The 8.3
   case used the form configuration. The 8.5 case used
   `tests/fixtures/external/config_compat/c10_e27/input.cf` with the common
   module from `tests/fixtures/native-export-commands/change/base`. On 8.5
   we also saved this configuration with the platform, then loaded and
   applied its CF into a fresh source before taking the tabulated measurement.
   In both measured sources SQL returned `DataSize = 10531998` on both
   parts, with `(PartNo, DATALENGTH(BinaryData))` equal to
   `(0, 10000000)` and `(1, 531998)`. The exported template was 10,050,000 bytes.
5. At each measured state run native and product
   `infobase config save [--db] <output.cf>`. Create a fresh target using
   the same native version, then run native `config load <product.cf>`,
   `config apply --force` and `config export <new-directory>`. Require exit
   code zero throughout. Compare all relative paths and their SHA-256 values.
   The table digest is SHA-256 of the concatenation, in Unicode path order,
   of each UTF-8 relative path (forward slashes), one NUL byte, and the
   32-byte file digest.

## 8.5 reference controls

An additional 8.5 source was exported immediately after a native XML import
that added metadata. In its applied view the pre-save export omitted two
entries from `ConfigDumpInfo.xml`. After load/apply the export included
the entries without version attributes. This occurred for the added common
module and for the binary template. In each case a control target loaded
from the platform's own CF produced exactly the same full tree as the product
target (6/6 and 8/8 files, respectively). The multipart row above uses the
fresh source populated from that native CF and compares directly with its full
export. A first attempt to apply the added template directly over the online
8.5 generation exited 1 and was excluded. The measured 8.5 multipart source
was prepared by exclusive apply in a separate database.

## Earlier measurement

On 23 October 2026 a separate BSP run reported 9,156 identical files after
load/apply/export on 8.5.1.1529, using the product built from `f9ebb8dd`.
Those files are outside the counts above. [Historical run](https://github.com/Untru/ibcmd-rs/issues/352#issuecomment-6359964546)
