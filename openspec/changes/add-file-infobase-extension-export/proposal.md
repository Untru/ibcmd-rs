# Export a named extension from a file infobase

## Why

Issue #520: native ibcmd 8.3.27.2214 exports an extension of a file infobase,
but ibcmd-rs rejects `--db-path` together with `--extension`. A clean-room
macOS run gives the same five files through Designer and ibcmd, including
ConfigDumpInfo.xml. This read-only route is separate from #348's remaining
extension import, apply and SQL 8.5 acceptance.

## What changes

- Read the file infobase extension registry with validated physical fields.
- Resolve exactly one named extension and its content-addressed image using
  the existing registry/CAS decoders and extension XML writer.
- Export with `infobase config export --db-path --extension` without invoking
  the platform, retaining the read lock through publication.
- Refuse unknown registry layouts, corrupt CAS, unsupported prepared states
  and unsupported options before publishing a tree.
- Add clean-room regression inputs and native 8.3.27.2214 evidence.

No extension writes, apply, all-extensions, EDT, new dependencies or changes
to the form writer are included.
