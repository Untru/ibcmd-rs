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
