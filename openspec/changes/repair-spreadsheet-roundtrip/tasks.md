## 1. Existing defect regressions
- [x] 1.1 Capture empty-projection and custom-font audit failures before fixing.
- [x] 1.2 Repair the planner and audit context, preserving rejection tests.

## 2. Evidenced palette and print members
- [x] 2.1 Capture clean-room round-trip failures for ScrollBar and pageSize/Auto accuracy.
- [x] 2.2 Implement only the evidenced tagged native representations with invalid-input coverage.

## 3. Acceptance
- [x] 3.1 Validate OpenSpec, narrow tests, physical codec policy and cargo xtask check.
- [x] 3.2 Run full offline ERP spreadsheet audit and independent review.
- [ ] 3.3 Build iter and retry full import only after resolving the changed failing set; record actual target state/time, leaving acceptance open on a further blocker.

Evidence: clean-room RED cases captured before each fix, then 138 focused tests passed. Physical-adapter policy guard and OpenSpec strict validation passed. Independent scoped review approved after the strict native-string correction. Full offline/runtime acceptance is still pending.

Final spreadsheet corpus: 13,583 packed/extracted/repacked/matched, zero differences or failures. Iter build and cargo xtask check passed. Full import passed the prior MXL failure, then refused an independent base-free DCS source round-trip mismatch; after 215.491 seconds ConfigSave remained empty and the active configuration generation was unchanged. Full-load acceptance is still open.
