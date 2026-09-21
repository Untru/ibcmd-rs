# Tasks

Every line here is measured against the ERP УХ corpus -- 12 507 forms, their
native 8.3.27.2214 export and the inflated bodies of the same database -- and
the number quoted is the share of stored records a candidate writer reproduces
byte for byte from the source alone. A writer refuses what it has not measured
rather than defaulting it.

## Done

- [x] **The body frame.** Ten members in all 12 507 bodies: root record, module
      text, attributes, parameters, commands, two appearance sections, `0`, `0`.
      *(body-frame-20260921.md)*
- [x] **The root record's head** -- 18 members. 12 469 / 12 469, refusing the 19
      forms that name a `<SettingsStorage>`. *(root-head-and-bag-20260921.md)*
- [x] **The root record's property bag** -- its shape: a count, that many
      `(key, value)` pairs, then exactly four members. Holds in all 12 488
      records that split. The keys belong to the form extension of the main
      attribute, one numbering per class.
- [x] **The root record's tail** -- 21 members after the two empty strings and
      the optional navigator. 12 410 / 12 410, refusing the 78 forms that carry
      a `<MobileDeviceCommandBarContent>`. *(root-record-20260921.md)*
- [x] **Event bindings**, of the form and of every item. 63 161 / 63 161.
      *(event-bindings-20260921.md)*
- [x] **Form attributes** -- the `{9,…}` record. 94 031 / 94 031 of the fixed
      part, with six members the caller supplies because they name
      configuration objects. *(form-attributes-20260921.md)*
- [x] **Form parameters** -- 24 863 / 24 863.
      *(form-parameters-and-commands-20260921.md)*
- [x] **Form commands** -- 60 983 / 61 228 (99.60%).
      *(form-parameters-and-commands-20260921.md)*
- [x] **The `{22,…}` container record**, across all nine kinds: head
      9 357 / 9 357, tail 9 354 / 9 357. *(group-record-20260921.md)*
- [x] **Colours** and **fonts** an item can carry.
- [x] The item payloads: label, input, check box, radio button, picture,
      spreadsheet, HTML, text and formatted document, usual group, button group,
      command bar, pages, page, popup, column group.

## Open

- [ ] **The property bag's values, per main attribute class.** The shape is
      read; what each key holds is not. A dynamic-list form writes key 1, a
      document form 2, 3, 4 and 24, a catalog form 0 and 24, a report form 5 to
      22 with 27 and 29.
- [ ] **The remaining item records** -- the `{37,…}` field, `{31,…}` button,
      `{12,…}` decoration and `{55,…}` table records, member by member against
      the source.
- [ ] **The two appearance sections** of the frame.
- [ ] **The settings blob** is *not* in the source. Two spellings account for
      11 842 of 12 507 bodies and nothing in the XML separates them, so the
      writer picks the canonical empty one and the round trip closes on the
      second export, not on the original database.
- [ ] **Wire the writer into compilation.** Nothing is wired yet: the base-free
      path still builds a seven-member frame and the blocker model still
      refuses.
- [ ] **Close the round trip**: export → load into an empty database → export,
      byte-identical.

## What the measurements keep turning up

Three properties are written **twice, under two different codings**:
`<VerticalScroll>` and `<Group>` in the root tail, and `<CurrentRowUse>` in a
command. Reading any of them consistently costs thousands of records. It is
worth assuming a fourth exists whenever a candidate stalls just short.
