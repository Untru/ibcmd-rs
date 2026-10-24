# Bounded Form field-map observation

This diagnostic supports a read-only investigation; it does not repair an export
or establish byte parity. Run it only after its exact source/binary has passed
independent review and targeted validation. The designated lab operator owns the
VM run. No raw source file needs to leave that VM.

## Input custody

Input is one source Form body's assembled raw-deflate BinaryData, not Form.xml,
not a CF container and not a synthetic reconstruction from XML. Bind it to the
failing source snapshot: record source identity, selected-row/part hashes and the
assembled input hash locally. Do not infer original FieldMap or required IDs from
an XML export. Use existing read-only extraction tooling; do not alter SQL rows,
trust policy, grants or the infobase. A CF-derived body is not proof of the SQL
body unless their exact row bytes are independently matched.

Keep all private values, selector arguments, binaries and failure artifacts in
the approved lab. Existing denied raw-source transfers must not be retried or
rerouted. Return only the fixed sanitized report and its provenance/receipt.

## Build and run

Build the example from the reviewed commit with the pinned toolchain and an
allocated build slot:

```text
cargo build --locked --no-default-features --example form_field_map_observer
<exact-example-binary> <one-local-source-form-body.bin>
```

Record exact commit, toolchain, target, features/profile, executable size/hash,
input size/hash and exit code. Supply no SQL credentials: this helper performs
no SQL connection. Run with an external wall-clock deadline and capture only its
own process, without interfering with other lab actions. Keep failed artifacts.
Do not run multiple candidate/native operations concurrently.

## Interpretation

The JSON contains fixed boolean/count fields and anonymous ordinals. It reports
field-map/required/order/selected-alias facts from existing parsers, not names,
IDs, query text, paths or raw values. Unavailable typed metadata is explicitly
not a resolved universe. Wildcard, incomplete or contradictory evidence must not
be interpreted as proof that the platform cannot resolve a field.

A marker cause remains undetermined until these facts and the corresponding
native observation establish it. Never remove all `~` markers or normalize the
acceptance comparison to make a diagnostic look successful. A later production
fix requires a failing clean-room reproduction and an exact-head native retest.

The Windows CI artifact contains `provenance.json` and the debug executable.
Accept it only when `builtCommit` equals the reviewed source commit and the
executable hash matches. A pull-request merge checkout may differ from the
requested head; use the matching branch-push artifact rather than relabeling it.
The artifact contains no source Form input or native acceptance result.
