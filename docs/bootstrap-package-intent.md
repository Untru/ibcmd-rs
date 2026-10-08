# Bootstrap package admission

Both `cf bootstrap` routes inspect the source metadata root with the canonical
XML namespace resolver before compiling or publishing. A directory must have
one package root among its root-level XML documents. An explicitly supplied XML
file is inspected as that document; the ordinary CF compiler still requires the
complete source directory. Package selection does not depend on the root file's
name or the requested archive suffix. Nested family and body documents remain
the responsibility of the selected source compiler.

Ordinary Configuration XML continues through the existing CF compiler and
publisher. `NamePrefix` (empty or nonempty) and
`ConfigurationExtensionCompatibilityMode` are shared ordinary Configuration
properties; they do not select Extension XML. Direct extension-only purpose,
adoption and ID-mapping properties (`ConfigurationExtensionPurpose`,
`ObjectBelonging`, `ExtendedConfigurationObject`, and
`KeepMappingToExtendedConfigurationObjectsByIDs`) select Extension XML.
ExternalDataProcessor and ExternalReport select their respective external
packages. Prefix aliases work through expanded names, while a foreign namespace
on a root or a recognized package property is refused. Duplicate recognized
package properties and duplicate Properties blocks are refused too. Legacy unqualified envelopes retain
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

The ordinary shared-property regression also exercises generated source → CF →
native XML → both bootstrap compilers → native XML for both `2.20` and `2.21`.
It preserves empty and nonempty prefixes and checks the authored shared
compatibility directly in the generated CF tuple, separately from the native
XML reading edition. An older ordinary root can store 8.3.24 while native XML
reports `ConfigurationExtensionCompatibilityMode` 8.3.27 or 8.5.1 for the target
platform; its own `CompatibilityMode` remains 8.3.24. The control then compares
the complete returned Configuration.xml bytes. It uses the native factory's
Russian script variant; the original English admission controls remain intact.
Explicit extension
purpose and mapping remain refused before publication even alongside these
shared properties. These generated controls do not substitute for acceptance
against retained native platform packages.

The legacy `{68,...}` compiler stores the independent own compatibility in
tuple field 26 and requested extension compatibility in field 43. Distinct
21/24, 19/12 and 10/27 native tuples confirm these coordinates; swapping them
would change the own compatibility when rebuilding XML that names the reading
edition. The base-free compiler's older `{67,...}` layout predates that shared
selector and keeps its own compatibility at field 43. Their native XML
projections use the existing platform policy; package admission does not
reinterpret either value or remove the ordinary property.


Ordinary `UsedMobileApplicationFunctionalities` values now use the root's own
named boolean flags in both source compiler routes. One shared schema roster
owns the existing native name/ID mapping; XML parsing resolves expanded names
before the base-free route's compact view discards namespace prefixes. A present
block must contain the complete ordered roster (37 entries in 2.17, 38 in
2.20/2.21), with one `functionality` name and one `use` value per entry. Names,
values, duplicates, extra fields and foreign namespaces are checked; malformed
or missing entries are refused before publication. Namespace aliases and the
existing unqualified legacy envelope are accepted. Omission of the entire
property retains the minimal Configuration contract. No missing flag is
replaced by the all-default reference block.

Generated allfalse, alltrue and mixed controls check physical tuple 53,
including the `{67,...}` trailing TextToSpeech scalar, and full returned XML
bytes through both compilers and both current dialects. Older physical reader
layouts and other property scopes that still admit only evidenced defaults are
unchanged. This source atom does not establish native-platform acceptance or
complete CFE/EPF/ERF builders.

XML 2.21 also projects fourteen 8.5 properties from older Configuration
layouts 67 and 68: eight empty auxiliary form references, empty captions,
and the evidenced navigation, theme, window-opening and migration defaults.
Their schema vocabulary is separate from the global Configuration default
table and the 76 layout. Both public compilers admit this older-layout
projection only at its exact values, after checking expanded names, attributes,
duplicates and scalar content. A nonempty reference or caption, an unknown
enum value, or markup refuses before publication instead of being omitted.
The 76 compiler retains its existing caption and paired-enumeration behavior.

The source XML dialect remains independent of the selected target platform and
the native layout. XML 2.21 does not by itself select a 76 tuple. Generated
round-trip controls preserve the complete returned XML; additional negative
controls preserve both absent and preexisting output files. These controls do
not establish universal native-platform acceptance for 8.5 configurations.
