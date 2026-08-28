# Proposal: emit the verified Form ListSettings tail

## Why

EDT writer evidence now proves the final two typed
`DataCompositionSettings` children delegated by Form `ListSettings`, while the
physical Form wrapper, prefix, indentation, and preceding complex sections
remain caller-owned. The physical adapter still emits those two children
manually.

## What changes

- Parse the committed DCS writer-evidence corpus through a bounded,
  fail-closed schema API.
- Add a schema-driven XML emitter for only `itemsViewMode` and
  `itemsUserSettingID`, using their verified order and default omission rules.
- Replace the two manual Form tail branches with that emitter while preserving
  the existing wrapper and complex filter/order/conditional-appearance output.
- Keep the aggregate DCS evidence matrix blocked on the four facts still
  missing from the corpus, while each full-preflight diagnostic reports only
  facts relevant to its envelope and opaque input.

## Impact

This change does not type or emit filter, order, conditional appearance,
opaque extensions, standalone DCS documents, wrapper QNames, or type IDs.
Canonical coverage remains exactly the existing two typed features.
