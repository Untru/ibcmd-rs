# Bootstrap package admission

Both `cf bootstrap` routes inspect the source metadata root with the canonical
XML namespace resolver before compiling or publishing. A directory must have
one package root among its root-level XML documents. An explicitly supplied XML
file is inspected as that document; the ordinary CF compiler still requires the
complete source directory. Package selection does not depend on the root file's
name or the requested archive suffix. Nested family and body documents remain
the responsibility of the selected source compiler.

Ordinary Configuration XML continues through the existing CF compiler and
publisher. Extension-only Configuration properties select Extension XML;
ExternalDataProcessor and ExternalReport select their respective external
packages. Prefix aliases work through expanded names, while a foreign namespace
on a root or extension property is refused. Legacy unqualified envelopes retain
the existing metadata-envelope contract.

CFE/EPF/ERF builders are not implemented by this change. Those inputs return
`bootstrap_package_not_supported`, with the artifact kind and source root path,
`ok: false`, no publication, and no output archive. A storage-header override or
`.cf` output name cannot turn them into ordinary CF. A Configuration cannot be
published with a recognized CFE/EPF/ERF suffix either. Other CF output suffixes
retain their existing behavior.

This is the first admission atom for issues #437 and #438. It prevents false
success but does not deliver their requested builders. Identity graphs,
scope-specific service entries, CFE configinfo/adoption, external reference
context/copyinfo, body compilation and actual native acceptance remain separate
implementation work. No new source size, file-count or depth quotas are added.

Generated controls are in `tests/cf_bootstrap_package_intent.rs`; XML namespace
controls are in `ibcmd-xml::metadata::package`. They use hand-authored source
strings and actual public CLI commands, with an empty PATH. No foreign fixture,
native executable, SQL connection or existing archive is needed.
