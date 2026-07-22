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

Portable fixtures cover minimal Subsystem content/hierarchy, child-rich
ExchangePlan and BusinessProcess tabular metadata, Task addressing ownership,
deterministic deflate output, profile fail-closed behavior, and strict native
inventory decoding.
