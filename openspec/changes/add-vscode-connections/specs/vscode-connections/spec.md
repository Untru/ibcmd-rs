## ADDED Requirements

### Requirement: Editor connections share CLI targets

The extension SHALL write server, database name and platform to workspace
`ibcmd-rs.toml`, preserve unrelated text, and reject a conflicting saved snapshot.
Friendly names, auth preferences and folder bindings SHALL use folder settings.

#### Scenario: An editor adds a database

- **WHEN** Add Connection completes
- **THEN** the actual `settings show` command sees the new database
- **AND** the TOML and workspace settings contain no password

### Requirement: Passwords use SecretStorage and explicit authentication

SQL passwords SHALL use SecretStorage and stdin RPC only. A protocol 1.1
`infobases/connect` MAY name `auth: windows` or `auth: sql`; Windows SHALL ignore
legacy login defaults and reject credential parameters; SQL SHALL require an
explicit nonempty user. Omitting auth SHALL retain protocol 1.0 behavior.

#### Scenario: Windows login with global SQL defaults

- **WHEN** the client connects with `auth: windows` while db-user is configured
- **THEN** the session uses integrated authentication without a SQL user/password

### Requirement: Status belongs to the current selection

The status bar SHALL show the selected friendly name, platform and pending state.
An inconclusive verdict or unsupported/failed pending request SHALL show unknown.
A crashed process SHALL show disconnected, and late responses from a replaced
selection SHALL NOT change the current status.

#### Scenario: The old connection answers after a switch

- **WHEN** a second connection is selected while the first request is unfinished
- **THEN** only the second connection can update the status bar
