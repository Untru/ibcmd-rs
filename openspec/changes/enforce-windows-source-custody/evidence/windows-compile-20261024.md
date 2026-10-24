# First Windows compile failure and scoped correction

Published Draft #533 head7343c38c1e690e59cdd5d93fb2c2c6122a68e88b, base6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4. Ordinary CI run40077844629:

- Portable Windows job120824977670 failed its release-shaped root all-targets check.
- Legacy Root Windows job120824977542 failed its root all-targets platform-oracle check.
- Both report E0432 at files/windows_profile.rs:9:33: DRIVE_FIXED is not in windows_sys::Win32::Storage::FileSystem. These are actual owned compile failures; root tests/runtime did not pass. Other warnings are not classified as pre-existing here.

Source correction2070c830907558ca14ccaf925f6dc43f2a201401, tree d4bf7220a101b06d18fa457b82dd51b36245adb3, removes that import and declares the documented SDK GetDriveTypeW return value DRIVE_FIXED=3 locally. [Microsoft API provenance](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew) specifies the value. No Cargo/dependency/feature, source API, supported-profile predicate, sharing/custody rule or test changes are included. Standalone pinned rustfmt and diff checks passed; no local compiler, VM, native platform or manual dispatch/rerun was used for this correction.

The failed fixed843 remains immutable, and test-first baseline751 is unchanged. Source-only approval of the earlier tree and local macOS check cannot substitute for Windows compilation. The seven new Windows controls remain unproven until exact-harness runtime receipts establish their outcomes. Current first paired run40077844741 is a separate gate; its actual outcomes must be retained and reported independently rather than inferred from these root-check failures. Draft remains Draft; full CI/acceptance and end-to-end Windows partial/MCP are not claimed.
