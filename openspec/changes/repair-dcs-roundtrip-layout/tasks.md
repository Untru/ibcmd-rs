## 1. Reproduce

- [x] 1.1 Pin supported compact type/settings layout against independent native fixture output and capture failure before the fix.
- [x] 1.2 Pin strict rejection of changed scalar values, character references, mixed content, CDATA and preserved whitespace.

## 2. Repair

- [x] 2.1 Compare only structural indentation loosely, keeping other source spelling exact and leaving writer input unchanged.
- [x] 2.2 Run covering DCS, spreadsheet and SQL compatibility tests, repository checks and independent review.

## 3. Validate

- [x] 3.1 Verify the originally failing source offline without publishing application artifacts.
- [ ] 3.2 Repeat the authorized full configuration import with verification and separate timing/state receipts; keep acceptance open if another blocker remains.

## 4. Bound unused namespace handling

- [x] 4.1 Reproduce the next failed DCS with a clean-room unused-prefix fixture and strict negative controls before implementation.
- [x] 4.2 Add a DCS-only conservative unused-prefix comparison, preserving both existing comparison APIs and every other lexical value.
- [x] 4.3 Verify the failed schema and all ERP DCS templates offline, run checks and obtain implementation review before another database import.
