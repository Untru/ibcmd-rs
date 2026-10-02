# Решения и отложенные мелочи планов 1–2 (из журналов исполнения)

Журналы `.superpowers/sdd/*` удалены после сбора; строки перенесены дословно.

## docs/superpowers/plans/2026-09-28-external-epf-erf-export.md

- Task 1: Ruling: rename VARIANTS reduced to ["Object", ""] (plan Task 5 had TabularSection/Row too) — native ext-cfg dumps keep DataProcessorTabularSection[Row].X.T for external processors (44 hits, 0 External*TabularSection) — cost if wrong: tabular-section type names mis-spelled in output
- Task 1: complete (commits e1a1d0b..a3d7633, tests: cargo test --no-default-features --test external_export → 0/2 pass, RED by design; Ruling: task-done not used for a red-by-design task — the plan's Expected is FAIL — cost if wrong: none)
- Task 2: Ruling: mod.rs declares only implemented submodules (plan said empty stubs for the rest) — stubs would be dead files between commits — cost if wrong: none
- Task 6: Ruling: fixture MXL template replaced by a non-empty compiled one — the upstream MXL writer refuses an empty 0-column spreadsheet (mxl.writer.format-plan-incomplete), an upstream gap (#319 area) outside the adapter; empty templates in real .epf will surface as a failed entry — cost if wrong: empty-template epf export reports failure
- Task 7: complete (no code: corpus shows 77/77 ObjectModule.bsl and 68/68 Help.xml byte-identical; Ruling: plan's storage-version hypothesis came from the throwaway spike, the real adapter already routes <ObjectId>.0/.1 — cost if wrong: none, corpus-verified)
- Task 8: Ruling: copyinfo references go through an explicit mssql_dump::ForeignReferences parameter instead of the plan's synthetic TYPES_ROW — object refs (pictures, style items) have no XML-row path into the index, one mechanism for both is simpler — cost if wrong: ~12 lines of upstream signature churn to rebase
- Task 8: Ruling: long tail (28 files) left for later — each remaining group needs configuration metadata that copyinfo does not carry (dynamic list fields, commands) or is an upstream writer gap — cost if wrong: 1.8% of corpus files differ
- Task 8: Ruling: corpus pair ОценкаПроизводительности (DataProcessor+Report with the same name) excluded — native dumps collided in one folder — cost if wrong: none
- Final: Ruling: no_copyinfo variant asserts equality with the base expected tree, not the platform's dump — the platform renames a copyinfo-less object to <Name>0 (verified in a fresh infobase), a quirk of a file it never writes — cost if wrong: none
- Final: Ruling: parse_owner_form_ref runs on the configuration path too (reviewer Minor 7) — fallback only fires when form_refs lacks the uuid, which never happens in a configuration; verified byte-identical .cf/.cfe exports vs base by the reviewer — cost if wrong: a config form slot naming a CommonForm the storage lacks would now be written instead of left empty
- Final: minor (deferred): object named ConfigDumpInfo loses its root XML; object named DataProcessors/Reports wipes output (finish name collisions)
- Final: minor (deferred): rename touches user text (synonyms/help) matching the object reference spelling; remove_root_property silent on formatting drift
- Final: minor (deferred): export report — fabricated copyinfo entry when absent, physical_entries off by one, synthetic main-row packed_bytes, internal paths in failure messages
- Final: minor (deferred): entries_of copies every payload before detection
- Final: minor (deferred): parse_owner_form_ref test appended to 79k-line tests.rs (rebase conflicts)
- Final: minor (deferred): ratchet ignores export failures/extra files; temp dirs not cleaned

## docs/superpowers/plans/2026-09-28-load-onto-base.md

- Ruling: plan 2 executed in worktree ../ibcmd-rs-load on branch onecdec/load-onto-base (reviewer of plan 1 builds the main checkout) — cost if wrong: one merge
- Task 1: complete (commits abfdc44..661d564, tests: cargo test --lib load::tests → diff test pass; Ruling: keys_by_output covered by the Task 3 integration test instead of a unit test — the report struct is verbose to hand-build — cost if wrong: a mapping bug shows only in integration)
- Task 3: Ruling: overlay storage profile is storage:mssql-config-configsave (plan said storage:cf-cli) — the legacy module/form compiler refuses any other profile, cf overlay uses the same default — cost if wrong: none, tests + platform check pass
- Task 3: Ruling: module text normalised to CRLF (not in plan) — the platform dumps CRLF; an LF edit from VS Code would otherwise not round-trip — cost if wrong: an intentional LF in a module becomes CRLF
- Final: Ruling: Form.xml editing dropped from load v1 (spec step 3 promised it) — the upstream form packer carries only a whitelist of properties and a check by our own re-export would not catch what it changes (packer and exporter can agree and both differ from the platform) — cost if wrong: users edit forms in the Designer until a verified path exists
- Final: Ruling: no structured `unsupported: [...]` field in CfLoadReport (part of reviewer's Important 4) — the capped message already names the paths; the report schema stays as plan 2 defined it — cost if wrong: a tool that wants the list parses the message
- Final: minor (deferred): .cfe base fails late (after a full export) with a glued message and no hint — note: .cfe load was later enabled on onecdec/offline-epf-cfe (9112a91), so only the late check remains
- Final: minor (deferred): --source-version mismatch (tree exported with 2.21, loaded with 2.20) gives no hint, lists every XML file
- Final: minor (deferred): .bin→.bsl pairing is not limited to *Module stems (Template.bin→Template.bsl would be treated as a module)
- Final: minor (deferred): ConfigDumpInfo.xml blocks a second load onto a previous load's output (configVersion changes)
- Final: minor (deferred): a wrong --base (newer than the tree) is not detected and reverts newer module text
- Final: minor (deferred): the report shows only counts, not the applied paths
- Final: minor (deferred): temp export dir not cleaned on panic/Ctrl+C; unit test tmp() dirs left in %TEMP%
- Final: minor (deferred): a form module cannot be added to a form that has none (4/253 corpus forms)
- Final: minor (deferred): verify_load.py — never edits Form.xml, assumes CommonModule for cf, continues after failed export, splits baseline on whitespace
- Final: minor (deferred): error context of module reads lacked the path (now covered by module_text for all module reads)
