# Tasks: platform 8.5 live activation

- [x] 1. Inventory every 8.3.27-specific storage, XML, activation, and RAC assumption and define the 8.5 evidence matrix.
- [x] 2. Build a disposable 8.5 BSP SQL clone and capture native/source/SQL baselines without mutating the registered source infobase.
- [ ] 3. Implement explicit 8.5 storage-profile detection and the minimum evidenced codec/staging differences, preserving 8.3.27 behavior.
  - Partial: exact 8.5.1.1150 initial five-row CommonModule dynamic publication is supported, with independent native/own source, storage and old/new session evidence. Other exact builds refuse before SQL. Standalone main/extension/LIVE/worker and repeated generations remain closed; this does not complete the storage-profile matrix.
- [ ] 4. Validate and adapt main module, managed form, extensions, and dedicated-worker handoff on 8.5.
  - Partial: initial existing CommonForm module-only publication is independently measured on native/OWN twins. Complete bounded DEFLATE, measured V4/layout59 and exact inflated tuple reconstruction bind every nonmodule byte. All 12336 raw source files are exact; full storage has seven explicit classified differences and no unclassified differences, and actual old A/A/new B/B cohorts overlap. Nested forms, extensions, repeated generations, standalone main/LIVE/worker and the rest of this task remain open; no same-session refresh is claimed.
- [ ] 5. Add profile, CLI, fixture, regression, and integration tests plus 8.3.27/8.5 timing evidence.
- [ ] 6. Build release artifacts, verify the compatibility report, restore/remove the laboratory clone, and commit atomic changes.
