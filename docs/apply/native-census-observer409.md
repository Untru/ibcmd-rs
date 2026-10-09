# Original census observer

The census collector is bound to its original process before its observation
can exclude that collector from the owned graph. This does not grant Ready,
add authentication denial fingerprints, release a lease or reinterpret a
previous UNKNOWN. Server authentication remains a separate prerequisite.

The private-root literal appears in the PowerShell census command itself.
Consequently the previous script placed its own process in `owned`, although
that observer was a short utility started by Rust, not an agent descendant.
ROOT reproduced this with a readonly command: the sole extra owned PID was
the SAME original census utility. This evidence identifies a census defect;
it does not prove the reason for every earlier AUTH refusal.

The script now publishes one separately typed `observer.identity` row for
its own `$PID`. That selection is provisional: it never excludes a process
by a shared image/name/root/parent alone. The Rust caller reserves a private
record and the expected collector vector index before command dispatch. Only
the SAME appended OriginalChild, with its retained process handle, exit0,
direct/BOTH and exact stdout bytes, can bind that row. Binding validates the
Rust parent, handle creation time inside the full CIM microsecond interval,
and selected image; the exact observed command hash is retained with it.

Observer validation rejects seed/descendant/listener/duplicate cases. Every
other private-root marker and foreign/opaque process follows the prior graph
and listener checks. The observer is never returned as an owned shutdown row
and cannot become a signal target. Observer/pending/bound records and original
collectors remain with NativeRuntime on refusal. Deadline fences consume the
original short-command deadline, including recovery, rather than creating a
new budget or reusing an expired startup deadline.

Eight consumer regressions cover valid separation, forged full identity and
boundary intervals, unrelated PowerShell/root/foreign processes, strict JSON,
and retention of original custody through errors, partial completion and panic.

Local validation of the integration on master `88eaa76` used Rust 1.95 with
locked offline dependencies and default features disabled. Formatting and
workspace/all-targets compilation passed. The library suites passed 84 managed
worker, 29 source-change and 43 activation methods, including all eight observer
regressions. The separately selected real readonly control passed once in 2.83s.
These results cover this integration; they do not establish server AUTH,
registration/load, SQL consumption, handoff/shutdown or completion of issue #409.

## ROOT-only actual readonly control

The ignored Windows test is:

`mssql_managed_worker::native::census_observer_tests::root_selected_readonly_census_observer_original_handle_and_both`

ROOT supplies an ordinary bounded UTF8 JSON file and its exact uppercase SHA256
through `IBCMD_CENSUS_OBSERVER_ROOT_REQUEST` and
`IBCMD_CENSUS_OBSERVER_ROOT_REQUEST_SHA256`. Its closed fields are:

```json
{
  "protocol": "native-auth409/census-readonly-request-v1",
  "powershell": {"path": "ROOT selected absolute path", "bytes": 1, "sha256": "ROOT actual uppercase SHA256"},
  "root_marker": "F:\\ROOT selected parent\\census-observer-ROOT-selected-canonical-nonnil-UUID",
  "seconds": 120
}
```

This is a schema sketch, not a runnable request or origin claim. ROOT chooses
actual values and pins them. The unique marker is used only as a search string;
no directory/registry/server is created. Selected seeds are empty and selected
port 0 has no valid TCP listener. The actual production script calls readonly
Get-CimInstance and Get-NetTCPConnection through one original utility. Its
response must contain the real observer and no marker-owned rows/listeners.
The test binds the real handle/BOTH, then refuses a forged parent against that
same original. No helper/mock positive OS census, native mutation, SQL, kill,
retry or ownership admission is used. One original 120-second deadline is selected
before environment/request/pin reads and preserved through final checks.

ROOT must separately retain actual command/tool/source pins, original test
terminal and BOTH receipts. A test pass is a readonly census control, not AUTH
4/4, production Ready, runtime recovery or acceptance of full issue #409.

Use the library target, for example `cargo test --locked --offline -p ibcmd-rs
--lib --no-default-features mssql_managed_worker:: -- --test-threads=1`.
The binary target does not contain this library test registry; zero selected
tests are not validation. Add the full ignored test name and `--ignored --exact`
only for the separately prepared readonly request.

Create the request separately before the original test invocation: the current
wrapper command line must not embed the marker literal. Otherwise that wrapper
is correctly treated as another unrelated private-root marker and refused.
