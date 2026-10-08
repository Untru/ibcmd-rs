# Empty ExternalDataSource CF support

The base-free descriptor compiler and CF exporter share the empty
ExternalDataSource codec. It preserves Manager, TablesManager **and** CubesManager
TypeId/ValueId pairs, localized Synonym, Comment, DataLockControlMode and the
explicit empty ChildObjects container. The native body stores all three pairs;
exporting only the common header loses required information.

`ibcmd-schema::external_data_source` owns the physical layout, collection IDs,
category names and enum mapping. The source adapter in `ibcmd-xml` reads the
existing canonical model with actual namespace bindings and source policy.
Original-source compilation retains that admitted canonical envelope through
physical encoding. Root-scoped or nested aliases for the metadata, readable
and core namespaces therefore keep their actual expanded-name meaning; the
compiler does not reconstruct bindings from prefix spelling or reserialize the
object subtree without its original root. The decoded host-owned DOM audit
route still uses the established writer and the same strict canonical admission.
The host compiler uses its established Header, Brace and XML writer. Physical
export delegates the whole object to this codec before its general fallback.
ScheduledJob also uses native code 2, but its direct header and three-field
outer envelope differ from the ExternalDataSource wrapper and six-field outer
envelope. Complete layout and own UUID checks prevent conflating the families.

The implemented shape has three empty native child collections. Nonempty
Tables/Cubes/functions and unknown source fields refuse compilation instead
of being discarded. Unknown or damaged declared EDS rows refuse strict source
export instead of producing a header-only document. The ordinary source route
adds no file-size, total-size or row-count limit. Its lexical preflight checks
the depth of this fixed empty-family grammar before materializing a Brace tree.

XML dialects 2.20 and 2.21 and enum codes Automatic=0, Managed=1,
AutomaticAndManaged=2 have generated source tests. These tests use fresh
hand-authored identities and sources. They do not establish native acceptance
for codes 1/2 or for platform 8.5. The complete Configuration XML ChildObjects
projection is the shared #440 prerequisite; EDS belongs before IntegrationService.
This change does not supply a separate root order or writer.

Layout facts were independently checked against retained native 8.3.27.2214
and 8.5.1.1529 CF/XML exports. Their empty Automatic descriptors have identical
packed and unpacked bytes; their XML dialects are 2.20 and 2.21 respectively.
This is a read-only layout comparison, not native acceptance of this compiler.
The frozen lab receipts retain the primary byte hashes and read-only extraction.
Foreign CF/XML fixture bytes remain in the laboratory
and are not distributed in these generated tests. Additional enum/layout facts
were checked against the MIT-or-Apache-2.0
[morph1c source](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7).
This implementation reuses facts with the existing host model and does not
import another metadata IR or CF pipeline. Pinned source references and primary
hashes accompany the #435 implementation handoff.

Issue #435 remains open until the public routes and fresh native load/export/
rebuild acceptance pass. Native acceptance must check all three generated
pairs and the entire EDS XML, each advertised enum value, edits preserving
generated identities, and the separately admitted platform profiles.
