## ADDED Requirements

### Requirement: The server speaks framed JSON-RPC over stdio

`ibcmd-rs serve --stdio` SHALL exchange JSON-RPC 2.0 messages over stdin and
stdout in `Content-Length` frames as the Language Server Protocol frames them,
SHALL write nothing but frames to stdout, and SHALL open no network port.

#### Scenario: A body that is not JSON

- **WHEN** a frame's body is not JSON
- **THEN** the server answers error -32700 with a null id and keeps serving

### Requirement: Initialize checks the protocol version

`initialize` SHALL serve a client whose protocol major version equals the
server's and whose minor version is not above the server's, and SHALL refuse
any other with error -32010 naming the server's protocol and executable
versions. Every other request before `initialize` SHALL fail with -32002.

#### Scenario: A client of another major version

- **WHEN** a client initializes with protocol `2.0` and the server speaks `1.0`
- **THEN** the answer is error -32010 with `serverProtocolVersion` `1.0`

### Requirement: The server calls the command line's functions

Every method SHALL run the library functions a command runs, so that each
editor action has a command-line equivalent: `objects/export` the export of
`mssql-dump-config` restricted to the objects' rows, `objects/import` the
stage of `mssql-stage-source-objects --path-prefix`, `config/pending` the
check of `mssql-apply-check`, and `tree/children`, `objects/status`,
`source/read`, `objects/export` the `ibcmd-rs objects` subcommands.

#### Scenario: Selected objects of an offline infobase

- **WHEN** the editor exports every object of a rows-folder infobase built from
  a CF file
- **THEN** the files written equal `cf export` of that file (but
  `ConfigDumpInfo.xml`) and `mssql-dump-config --rows-dir --object` byte for
  byte

### Requirement: Reads follow the database

The server SHALL keep what it read of an infobase only while the digest of its
`versions` row stays the same, and SHALL read the tree level by level.

#### Scenario: The configuration changed in the database

- **WHEN** the `versions` row differs from the one the cache was built for
- **THEN** the tree and the exported files are read anew before the answer

### Requirement: Long requests can be cancelled

A request named by `$/cancelRequest` SHALL not start when it is still queued
and SHALL stop at its next step when it runs, answering error -32800.

#### Scenario: An export queued behind a comparison

- **WHEN** the editor cancels an export that waits behind a comparison
- **THEN** the export answers -32800 and writes no file

### Requirement: The password stays in memory

A SQL Server password SHALL reach the server only as a parameter of
`infobases/connect` or through `IBCMD_DB_PSW`, and SHALL never be written to a
file, a response or a `log` notification.

#### Scenario: Connecting with a password from the editor's secret storage

- **WHEN** the editor connects with `user` and `password`
- **THEN** the answer and the `log` notifications name the login, never the
  password

### Requirement: Rows folders are read-only

An infobase read from a folder of stored rows SHALL answer `objects/import`
and `config/pending` with error -32013.

#### Scenario: Import into a rows folder

- **WHEN** the editor imports objects into a `rows-dir` infobase
- **THEN** the answer is error -32013 and nothing is written
