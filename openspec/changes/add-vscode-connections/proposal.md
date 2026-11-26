# VS Code workspace connections (#378)

## Why

The stdio server already lists and opens infobases, but VS Code has no connection
client. An editor-created database must remain visible to the CLI without storing
its password in project files.

## What Changes

- Add a minimal TypeScript VS Code extension: Add/Edit, workspace bindings,
  switching, SecretStorage and a platform/pending status bar.
- Keep database targets in shared TOML and editor-only labels/auth metadata in
  folder settings. Preserve TOML comments and reject conflicting saves.
- Add optional explicit `windows`/`sql` authentication to connect, protocol 1.1,
  preserving the legacy fallback for 1.0 clients omitting `auth`.
- Test actual CLI/stdio and extension-host flows separately from synthetic status
  failure/race fixtures. Object UI, apply actions and distribution remain separate.

## Impact

New `editors/vscode` client. Additive editor RPC parameter; no TOML schema change.
