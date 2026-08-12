# Hierarchical and workflow metadata — platform 8.3.27.1989

This note records the independent evidence used by the base-free native
compilers for `Subsystem`, `ExchangePlan`, `BusinessProcess`, and `Task`.
Neither compilation nor tests invoke 1C, EDT, a JVM, or a base artifact.

## Evidence pairs

The strict layouts were derived by comparing readable XCF with raw-inflated
`Config` rows from the 8.3.27.1989 lab corpus:

- `Subsystem.Администрирование`, UUID
  `6b8ea295-dbfb-4fe2-91fd-a57179d300c3`;
- `ExchangePlan.Мобильные`, UUID
  `1a8e1ee3-4518-47a0-87a8-566feb48f243`;
- `BusinessProcess.Задание`, UUID
  `dad11c2e-08fc-4a6b-8829-8be6c64c15fc`;
- `Task.ЗадачаИсполнителя`, UUID
  `3ad08f4a-6202-4099-b6cc-bc116e6731a0`.

The legacy strict readers in `src/mssql_dump/mod.rs` provide a second,
independent executable description of the same owner fields, collection
markers, generated-type slots, and child wrappers. The new compiler does not
call those readers.

## Profile-selected layouts

The 8.3.27.1989 profile selects four independent constants:

- `subsystem-v1-crlf-utf8-bom`;
- `exchange-plan-v1-crlf-utf8-bom`;
- `business-process-v1-crlf-utf8-bom`;
- `task-v1-crlf-utf8-bom`.

A future platform build must add a new profile/layout implementation. The
compiler never derives a native layout from the XML dialect or compatibility
mode.

## Native root and collection evidence

| Family | Owner discriminator / fields | Root collections |
| --- | ---: | --- |
| Subsystem | `22` / 9 | child subsystems `37f2fa9a-b276-11d4-9435-004095e12fc7` |
| ExchangePlan | `37` / 51 | attributes, templates, tabular sections, forms, commands |
| BusinessProcess | `30` / 49 | templates, forms, commands, attributes, tabular sections |
| Task | `33` / 52 | templates, forms, attributes, addressing attributes, reserved, commands |

Generated-type inventories are exact: five pairs for ExchangePlan and Task,
six for BusinessProcess (including `RoutePointRef`), and none for Subsystem.
Direct and nested attribute wrappers, tabular-section wrappers, form/template
UUID references, command identities, Task addressing dimensions, Subsystem
content, and child-Subsystem references are validated before emission.

## Deliberate support boundary

The XML codec accepts either an empty `StandardAttributes` element (platform
defaults) or the exact shared default property bag. Customized standard
attribute bags and complex design-time values are rejected. The native
compiler emits the evidenced shared default descriptor and never silently
drops unsupported customization.

Flowchart and source assets remain separate storage artifacts; this issue
covers the primary metadata rows and their identity/ownership references.

## BusinessProcess attestation on 8.3.27.2214

Issue #282 adds an immutable real-object pair for
`BusinessProcess.Задание` at
`tests/fixtures/native-evidence/8.3.27.2214/business-process-duty`. The exact
native CF element is byte-identical in the parent CF and the isolated lab CF
saved by `ibcmd 8.3.27.2214`; its native XML 2.20 export is also byte-identical
to the paired source snapshot.

This evidence confirms the 49-field owner, all five ordered collection markers
and counts `0/4/0/27/0`, `UseStandardCommands=true`, and the six generated
types through `BusinessProcessRoutePointRef` in schema/native XML order. Empty
Template, Command, and TabularSection collections remain fixture-specific; no
non-empty encoding for them is inferred here.

## Task attestation on 8.3.27.2214

Issue #317 adds an immutable real-object pair for
`Task.ЗадачаИсполнителя` at
`tests/fixtures/native-evidence/8.3.27.2214/task-assignee`. The exact native CF
storage element was extracted before compilation, then independently preserved
by `ibcmd 8.3.27.2214` while saving an isolated lab configuration. Its paired
native XML 2.20 export is byte-identical to the source snapshot.

This evidence confirms the 9-field Task root, discriminator `33`, 52 owner
fields, all six ordered collection markers, nil internal UUID slots 13/14, and
an empty Task `Templates` collection on 8.3.27.2214. It deliberately does not
claim an encoding for a non-empty Task template collection.

## Register and plan generated-type attestation on 8.3.27.2214

Issue #282 also carries a compact diagnostic corpus at
`tests/fixtures/native-evidence/8.3.27.2214/register-generated-types`. One
minimal XML 2.20 configuration contains an `AccountingRegister`, a
`CalculationRegister`, their required plans, and one shared recorder document.
The first platform-saved CF is only 90,645 bytes. Loading it into a second
isolated file infobase changes the outer CF hash, but the four selected exact
raw payloads and native XML exports remain byte-identical. The initially saved
CF was therefore reused to attest both plan objects without another import or
full export.

The seed follows Unica main at
`a527d40962d047c6922c903b37510b30f697da42`, but Unica is not the authority.
The first seed exposed an internal Unica inconsistency: the chart-of-accounts
writer emitted `MaxExtDimensionCount=3` by default while its own hint logic
treated the default as zero. Platform 8.3.27.2214 rejected that source without
`ExtDimensionTypes`; the attested seed therefore states
`maxExtDimensionCount=0` explicitly. Unica's public Rust handler on that same
head already emits zero and has a focused test, so no production Unica defect
is claimed. The discrepancy is retained only for review of its legacy
model-equivalence fixture; a pull request is warranted only if that fixture is
still required to match the public handler for this object family.

The accepted double round proves seven generated types for each register. The
accounting order begins with `Record`, `ExtDimensions`, `RecordSet`, and
`RecordKey`, then ends with `Selection`, `List`, and `Manager`. The calculation
order is `Record`, `Manager`, `Selection`, `List`, `RecordSet`, `RecordKey`, and
`Recalcs`. The offline regression compares the complete generated-type writer
block against native XML, including every type/value UUID. No recalculation
child layout or unrelated register property is inferred.

The same evidence proves seven generated types for `ChartOfAccounts`, including
`ExtDimensionTypes` and `ExtDimensionTypesRow`, and eleven generated types for
`ChartOfCalculationTypes`. The XML writer already emitted the complete native
blocks, but the raw type index for object code `32` exposed only the first five
ChartOfAccounts types. The evidence-backed schema now indexes all seven. This
fix is deliberately limited to the declarative generated-type slots and their
exact header/UUID-vector guard; a partial or malformed vector fails atomically.
No new raw layout or XML property heuristic was added.

Portable fixtures cover minimal Subsystem content/hierarchy, child-rich
ExchangePlan and BusinessProcess tabular metadata, Task addressing ownership,
deterministic deflate output, profile fail-closed behavior, and strict native
inventory decoding.
