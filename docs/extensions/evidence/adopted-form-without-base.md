# Adopted form without a base record

The clean-room extension in `tests/fixtures/external/v85_extension_inputs/adopted_form_no_base/input.cfe` contains an adopted catalog form. Native 8.5.1.1529 writes three event bindings (OnOpen Before, BeforeClose Override, OnCreateAtServer After) and a command Action with Before. The complete Form.xml contains no BaseForm. The CFE SHA-256 is `ba622624d2a87ded0e94d7d8f72ffa8c500db27d2d42cd66e0c406e9a5e5a917`.

## Reproduce the native comparison

On 2026-11-24 the synthetic adopted/form_events example was imported from XML with the base form section removed, a command added, and its empty BSL handler added to the form module. The native 8.5.1.1529 utility saved the CFE. To regenerate the oracle from that checked-in CFE, run the following PowerShell from the repository root. Set `$native` to the 8.5.1.1529 ibcmd executable.

```powershell
$work = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory $work | Out-Null
$db = @("--db-path=$work/db", "--data=$work/server")
$cfe = (Resolve-Path tests/fixtures/external/v85_extension_inputs/adopted_form_no_base/input.cfe).Path
& $native infobase create @db --create-database
if ($LASTEXITCODE) { throw "create failed" }
& $native infobase config load @db --extension=ТестРасширение $cfe
if ($LASTEXITCODE) { throw "load failed" }
& $native infobase config export @db --extension=ТестРасширение "$work/native"
if ($LASTEXITCODE) { throw "export failed" }
git diff --no-index --exit-code -- tests/fixtures/external/v85_extension/adopted_form_no_base "$work/native"
```

Reloading the CFE in an empty file database reproduced **9/9 native files**, including ConfigDumpInfo.xml. The integration test runs the public file export command on that same CFE. All **8/8 source files** matched the native tree byte for byte with no extras. The file exporter omits the database inventory, which the test checks separately.

```text
cargo test --locked --test integration extension_v85::adopted_form_interceptors_without_a_base_form
```

The 8.3.27.2214 file export profile also preserved the three event types and the command type. A fresh native Designer export of this particular CFE on 8.3.27.2214 requires a license unavailable on the measurement host. The original seven installed extensions and the ERP УХ reference databases from issue #414 remain outstanding.

## Native 8.3.27.2342, measured on 2026-11-24

The 8.3.27.2342 standalone ibcmd returned exit code 0 for create, load and export with the same CFE and a fresh empty file database. To repeat the measurement, set `$native` to this build and run the three native commands above in a new temporary directory. For this build compare the exported catalog Form.xml to `tests/fixtures/external/adopted_form_no_base_8_3_27_2342/Form.xml`. Its SHA-256 is `63d30cb8ce13005c4aadc691c7ccfb2531406edd97fd26fbb4e22dec9af2036e`. The XML contains the three stored event call types and the Before command action, with no BaseForm element.

| ibcmd-rs command on the same input | Comparison with native 8.3.27.2342 |
| --- | --- |
| `cf export $cfe "$work/from-cfe" --platform 8.3.27` | All 8 source paths and bytes matched at the parent of #475 (`f9ebb8dd`), its head (`4f977301`) and the PR base (`54b796f6`). Only the native database dump contains ConfigDumpInfo.xml. |
| `infobase config export "--db-path=$work/db" --extension=ТестРасширение --platform=8.3.27 "$work/from-db"` | All 9 files matched at `54b796f6`, including ConfigDumpInfo.xml. |

Both ibcmd-rs entry points reject the exact selector `8.3.27.2342` (exit 2). The successful runs used the supported family selector `8.3.27` (XML 2.20). The regression test `form_events::adopted_form_without_base_matches_native_8_3_27_2342` compares the complete form to the native bytes. The 8.3.27.2214 native run and the original seven-extension and ERP УХ lab matrix remain open.
