# Partial descriptor-delta correction for #418

The exclusive OWN Apply planner for 8.3.27.2214 previously chose deletion of
`Params.DynamicallyUpdated` whenever `ConfigSave` contained a descriptor. It
now retains the marker when every staged descriptor is fully decoded and its
complete decoded bytes equal the effective active descriptor. Compressed-byte
differences alone do not trigger deletion. A changed or newly created
descriptor retains the previous deletion policy under the existing admission
gates. Other platform profiles retain their previous policy.

This corrects an observed counterexample, not the complete native collection
rule. [Native body S](native-body-S-2026-10-02.md) stages a genuinely changed
module body and a recompressed, decoded-equal CommonModule descriptor; native
Apply retains all physical marker fields and its 156-byte payload. The
[historical F4 pair](f4-counterexample.md) supplies a small immutable binary
regression fixture. [Native-staged Comment/Synonym twins](own-native-staged-twins-2026-10-02.md)
show marker deletion for genuine descriptor changes, so unconditional retention
would discard already observed behavior.

The comparison uses the newest matching descriptor alias in the declared,
validated dynamic history, falling back to the ordinary descriptor. It does
not normalize whitespace, BOMs, text, headers or metadata properties. Complete
raw-deflate consumption, bounded decoding, the known descriptor parser and its
own UUID must all succeed. Multipart descriptors, malformed or foreign UUIDs,
unsupported alias inventories and undecodable streams refuse OWN planning.
Creation still passes through the existing structural admission gates.

All deciding blobs are bound to inventoried length, digest and physical header
before transfer. The generated transaction rechecks complete staged inventory,
ordinary descriptors (including absence), pending aliases/history markers,
ordinary `versions`, and the Params marker after table locks and before any
fold, publication or cleanup. This closes gaps left by the older aggregate
fingerprints, which did not include dates or attributes. Header or payload
drift aborts the transaction through the existing pending-inventory guard.

The descriptor capture and effective-alias helpers reuse the preserved
`feat/05-params-marker` investigation, historical commit
`f1963df0654275ad25821954c3cbda3e1c1a6d92`; this patch adds production binding,
planner regressions and transaction integration. The capture retains finite
resource budgets shared by active and staged reads. Exceeding a budget is an
explicit refusal, not a claim that arbitrary configurations are covered.

The SOURCE regressions cover recompression, genuine decoded changes, malformed
streams, header drift, effective alias selection and generated-guard ordering.
They execute no SQL and establish no native acceptance. #418 stays open until
the remaining native/OWN matrix is completed, including T20/T21, repeat and
effective pending overlays, with complete headers and bytes across all six
storage and ten auxiliary tables. No fixed 20/21-row trigger is implemented.
The `.ui` and native service-information differences remain separate measured
differences; this correction does not claim full Params parity or release 0.5
acceptance.
