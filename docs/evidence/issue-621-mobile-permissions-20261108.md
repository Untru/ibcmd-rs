# Configuration mobile permission messages

Refs #621. Base54b796f64da5daed3714acdf7810581fe0e58778;
public corpus8d138b896f462c3053d16076cbabf0ffe5030c1c.

## Rule

The functionality roster and permission messages are separate children of
UsedMobileApplicationFunctionalities. The roster remains complete and ordered.
Following messages have distinct permission IDs and localized descriptions;
they must not be counted as functionality entries or discarded after parsing.
Schema-owned message IDs map to the distinct older capability IDs:
0→29 (Biometrics),12→22 (Camera),13→23 (Microphone),14→24 (MusicLibrary),
15→25 (PictureAndVideoLibraries),16→26 (AudioPlaybackAndVibration),
20→35 (PostNotifications). Fourteen fresh native controls on installed2214
and authorized1529 proved those pairs from both complete stored descriptions.
A message does not enable its functionality: the controls retain disabled
capability flags while storing a description. No parser slot, dependency,
vendor adaptation or generated JSON change is introduced.

The XML parser validates the complete namespace-aware block, unknown names,
attributes, leaf shape, unique permission IDs and unique nonempty languages.
Description text retains whitespace and decoded characters. Both descriptor
and bootstrap writers retain messages; descriptor reading validates the
counted records and cross-checks the older capability descriptions.

## Portable development evidence

Before the fix, exact public Configuration.xml failed with InvalidEnvelope
complete-roster rejection, then the isolated writer refused permissionMessage.
The complete descriptor regression now covers both saved editions and eight
malformed stored-record mutations. Additional controls compare all seven
permissions, empty descriptions and multiple descriptions as complete XML.
Unknown attributes/namespaces, nested leaves, duplicates, empty languages
and whitespace-trim loss refuse. The latter was separately observed red
before tightening language admission.

The initial check completed nine PASS; policy passed without baseline changes.
Matched242-set base/fixed CF runs:8060 equal,5039 different,137 exporter
refusals on both sides. Matched124 stored sets retain7449 equal files with
identical equality/difference lists; no equality baseline is changed.
Two resolved mobile-class refusals leave round-trip expectations. Six combined
Configuration refusals now name the next exact #678 limits. Clean-base and
fixed audit runs on identical copies with only permissionMessage removed
reproduced the same height/TaxiEnableVersion8_5 refusals. The subsequent
strict round-trip suite completed four PASS. Those independent limits remain.

## Verification status

The temporary environment was removed during verification. The source was
recovered deterministically and checkpointed; completed historical checks
above are distinguished from the final gate. Final clean iter identity,
verified native import/apply/export, full staged CI and independent review
are pending and will be recorded after rerun. Saved1150 oracles remain intact;
no exact1150 runtime, SQL Server or full combined-configuration import claim.
