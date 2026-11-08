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

## Recovered portable verification

After scratch loss the source was restored and checkpointed. Fresh check9,
strict round-trip4, observed-problems1 and physical-policy guard pass; policy
and equality baselines are unchanged. The18 complete-Configuration control
variants and eight malformed-record mutations pass again.

Matched242 CF and124 stored runs use an identical feature test harness for
both the verified clean-base executable54b796f64d (SHA-256
bb7d2e0495455fa85f6420d71f13575a22de8f3b1e49856741d67c0b97a38b48)
and the fixed debug executable (SHA-256
c681c475a26c8ebbe05871fecb3e1297aaa518e23b57732814c682f127e99094).
The product executable is exchanged with try/finally restoration; no source
checkout is changed. Both sides pass, keeping8060/5039/137 CF results with
identical verdict/first-difference lists, and7449 stored equal files with
identical equality/difference/exit-code lists. No equality baseline changes.

Fresh base audits of both minimal native trees reproduce exactly one
complete-roster refusal each. Audit commands return0 with a failed report;
that command exit is not successful compilation. On identical combined
copies with ONLY permissionMessage removed, fresh clean-base and fixed
audits reproduce identical Configuration refusals:height on2214/1150,
height plus TaxiEnableVersion8_5 on1529. This independently confirms the
six next #678 expectations; it does not claim combined import succeeds.

## Fresh native controls and final candidate

All seven names on both builds, plus empty and multiple descriptions,
were regenerated through official create/import/apply/export/save:90 native
commands return0. Eighteen fresh CF root records prove the distinct ID map
above in BOTH older capability descriptions and the functionality tail.
Counts/IDs/description values agree with each native Configuration.xml;
functionality and capability use flags remain identical across every
message variant. Empty description is stored with description-present1 and
localized count0. Multiline content stores CRLF while native XML carries LF;
leading/trailing spaces and escaped characters survive. These are measured
native conventions, not expectations generated from the candidate.

Clean iter build `d2d8b11fbea9f81b7f9960007f7fb852464ecc03`, Rust1.95.0/macOS ARM64;
version `ibcmd-rs 0.4.1`, SHA-256
`dcee18fc1e4cfed6d04853203f92ef7bda5eef20b7d6c3b41e5e24766d9c6908`.
Version output does not embed a commit; the clean checkout and hash bind it.
Fresh runtime8.5.1.1529 is authorized in place of1150; both saved editions
remain offline oracles. No fixture is rewritten by this verification.

All18 final cases complete144 successful workflow commands. Every audit
compiles three descriptors/six rows with zero failures. All18 default-verified
imports and native applies succeed. Three complete application files in
four routes per case (CF,fileDB,native post-apply,own post-apply) match:
216/216 full-file comparisons across72 trees, without XML or whitespace
normalization. Each candidate uses empty PATH; native calls use
LANG=ru_RU.UTF-8 and distinct --data. No --no-verify is used. All input
CF/XML trees remain unchanged;36 source/candidate own-read database hash
checks preserve exact bytes.

CF/file readers additionally match all four source files, including
ConfigDumpInfo.xml. Same-candidate native/own exports match all four files
per case:72/72 additional direct comparisons including that manifest.
Against source manifests, exactly three configVersion values change;
strict byte comparison after replacing ONLY those attribute values is
identical. These service deltas are reported, not counted as whole-file
source equality. A separate official-native canonical XML reimport control
for ordinary,empty,multiple messages on both runtimes is recorded below.

| Runtime | Message case | Application-tree SHA-256 |
|---|---|---|
| 8.3.27.2214 | AudioPlaybackAndVibration | 63d3da002c60bd92aa7eca5674202cf69385e06234263783afd9d1c46d81f6d4 |
| 8.3.27.2214 | Biometrics | 76a89a0578f9fe276880d5cada3bffbfe198c7885e1be80d72562ed02ace23a8 |
| 8.3.27.2214 | Camera | 5162891915bd0c0f41d6c358d3db5d5fd12edbd1809648516807424569590fcb |
| 8.3.27.2214 | Microphone | 07171dd23d9540a1d2f8b117082dc53b27da45a84e94856eab8ae338154a0f22 |
| 8.3.27.2214 | MusicLibrary | 53227264a1a888f84ee030a082bd4df34b42bf4f8da79ea17046fae491f30a1b |
| 8.3.27.2214 | PictureAndVideoLibraries | 72837a4c11ba0da417d81416d23fed2f99871ff3ed8d177212a381ab2a95866c |
| 8.3.27.2214 | PostNotifications | 58ace4d43f92443e963612d3a00c1ab60014d2184d2352ccf5dedc95725ceb14 |
| 8.3.27.2214 | empty | 481ae96d0a333f59d5468477a93afc8a4843aa00b23f46c77c1d8712ac7fd19b |
| 8.3.27.2214 | multiple | 3ea8f9f14396a88f54753ef593d3190cb700c1970004e9facc7615d78177554e |
| 8.5.1.1529 | AudioPlaybackAndVibration | dfb40ade7a0c5309cff58db72352a5b729ff61ecec5e7f24d7b2a532aaa6c5f7 |
| 8.5.1.1529 | Biometrics | fd1c42330da3e1cd7c7e827532b6bc5ab26b7fbaa26df81770deed12ccbfe24a |
| 8.5.1.1529 | Camera | c66509f19c3d52a66b52f37e4c23aaad8d8484f01e8349fb9cccc57d9bf282ab |
| 8.5.1.1529 | Microphone | 94a080e7247babfee89cf8c62425cbb2ecb4602b9e138fc5a1bacf1602e0d7de |
| 8.5.1.1529 | MusicLibrary | 5d6fc83e6d595b925166aebe58b63d5d44343b7ea6790b36a29baddd3c457759 |
| 8.5.1.1529 | PictureAndVideoLibraries | 78848b2a8dd2f40d0becf04730c605164db66d17fd4d12d469c3b5b230941f00 |
| 8.5.1.1529 | PostNotifications | 890a86bb757de01588cc636b2f391a0bb2e5944108d60d3b38ebdae97c00d93e |
| 8.5.1.1529 | empty | df6203f866efbecd5c16790592cd9bc4e57d0e978b2d44253854c45b22f82f9c |
| 8.5.1.1529 | multiple | 2a57dedf48563145edca5accb59478f618c166623a68e208f43becfe53aaa2e4 |

Tree hashes cover sorted relative path,NUL,fileSHA256,NUL,byte length,LF;
only the generated manifest is excluded, as measured separately above.
The controls replace the public Biometrics permission name for the seven
cases; empty/multiple messages are exactly those in the portable full-XML
regression. No raw record, native export tree or database enters this commit.

```text
cargo xtask affected
cargo xtask check
cargo test --locked -p ibcmd-xml metadata::configuration_mobile::
cargo test --locked -p ibcmd-compiler configuration_projection_keeps_the_complete_permission_description
cargo test --locked -p ibcmd-rs --test integration coverage_storage_records::mobile_permission --no-default-features
cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features
cargo test --locked -p ibcmd-rs --test integration coverage_observed_problems:: --no-default-features
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1
cargo build --locked --profile iter
native ibcmd infobase create --db-path=<control-copy> --create-database --locale=ru_RU --data=<unique>
native ibcmd infobase config import --db-path=<control-copy> <public-source-variant> --data=<unique>
native ibcmd infobase config apply --db-path=<control-copy> --force --data=<unique>
native ibcmd infobase config export --db-path=<control-copy> <native> --data=<unique>
native ibcmd infobase config save --db-path=<control-copy> <control.cf> --data=<unique>
ibcmd-rs cf extract <control.cf> root <root-output>
ibcmd-rs cf extract <control.cf> <root-uuid> <configuration-output>
ibcmd-rs audit-empty-stage <native> <empty-rows> --platform=<build> --output=<audit.json>
ibcmd-rs cf export <control.cf> <cf-output> --source-version=<2.20-or-2.21>
ibcmd-rs infobase config export --db-path=<control-copy> --platform=<build> <file-output>
native ibcmd infobase create --db-path=<candidate-copy> --create-database --locale=ru_RU --data=<unique>
ibcmd-rs infobase config import --db-path=<candidate-copy> --platform=<build> <native>
native ibcmd infobase config apply --db-path=<candidate-copy> --force --data=<unique>
native ibcmd infobase config export --db-path=<candidate-copy> <post-native> --data=<unique>
ibcmd-rs infobase config export --db-path=<candidate-copy> --platform=<build> <post-ours>
```

Native binary SHA-256:2214
`1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`;
1529
`40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.
Six official-native canonical XML reimport controls (ordinary,empty,multiple
on both builds) complete24 native commands with exit0. All18 application
files match byte-exactly and only the same three configVersion attributes
change per manifest; strict raw comparison of every other byte passes.
Together with the90 original native oracle commands and144 final workflow
commands,258 control/matrix commands complete successfully.

Full staged `cargo xtask ci --keep-going` completes with terminal exit0:
29 PASS, no failed or skipped local step. Workspace4709, root integration555
and root library904 tests pass; existing ignored89/4/7 are unchanged.
Temporary-directory cleanup passes. All new source/tests were tracked before
the gate; subsequent edits only extend this evidence Markdown. No test was
ignored, relabelled or removed. Fresh-context review remains pending. No exact1150 runtime, SQL Server, full combined
import, local release build, SBOM or release-audit claim.
