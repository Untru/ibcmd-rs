## ADDED Requirements

### Requirement: Selective export publication
The editor SHALL export a selected object, group or multiselection using the main configuration's full-export bytes, reconcile selected ConfigDumpInfo entries, and preserve unrelated files and unsynchronized inventory versions.

#### Scenario: Group selection
- **WHEN** the user chooses a metadata group for export
- **THEN** its objects are staged with the same bytes as a native full export
- **AND** source files remain unchanged until client preflight completes

### Requirement: Local change protection
The editor SHALL list affected modified, staged, untracked and ignored Git paths before overwrite, require an explicit decision, and refuse dirty buffers or changed source fingerprints before publication.

#### Scenario: Unsaved editor contents
- **WHEN** an affected file has a dirty editor buffer
- **THEN** export refuses publication with the affected paths listed

#### Scenario: Local edits during confirmation
- **WHEN** an affected disk file changes while confirmation is open
- **THEN** publication refuses the stale plan

### Requirement: Native changed export
The editor SHALL select changed database objects using ConfigDumpInfo native versions, including native full-conversion fallback for deletion or rename, without advancing unrelated partial-export versions.

#### Scenario: Removed database object
- **WHEN** native version comparison detects an object removal
- **THEN** its obsolete descriptor and assets are planned for removal
- **AND** the current inventory is staged while source files remain unchanged
