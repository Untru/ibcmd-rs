## Implementation

- [x] Shared TOML add/edit with comment preservation and conflict detection.
- [x] Folder metadata/bindings, explicit authentication and SecretStorage.
- [x] Framed stdio lifecycle, cancellation and late-response isolation.
- [x] Switching/status bar with inconclusive/offline pending represented as unknown.
- [x] Unit and real CLI/stdio integration coverage.
- [x] Complete extension-host regression run and independent re-review.
- [ ] Live SQL/Windows authentication verification (requires a separately provisioned environment).

The final unchecked live verification is a documented verification gap, not a
claim that synthetic responses prove a login. No tree/apply/distribution work is
included in #378.
