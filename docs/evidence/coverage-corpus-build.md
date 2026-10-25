# Building the corpus regression sets

The additional `corpus-*` sets extend the generated Coverage configuration
from [PR #535](https://github.com/Untru/ibcmd-rs/pull/535). Inputs are complete
and committed: configuration base, metadata template, typed values, owned
form/module/template files and binary image assets. Authoring scratch scripts
and application exports are not required to rebuild them.

For each set, generate XML in a disposable directory, then prove it with the
matching installed platform:

```text
python tools/coverage/generate.py <set> <scratch/tree>
python tools/coverage/native_round_trip.py --ibcmd <native-ibcmd> --platform <exact-build> --xml-version <2.20-or-2.21> --set <set> --tree <scratch/tree> --work <scratch/round-trip>
```

Native proof imports and exports the generated input, saves CF, loads that CF
into another infobase, and exports it again. The two native exports must be
byte-identical outside `ConfigDumpInfo.xml`. Refused combinations remain
explicitly `platform_refused`; they never enter the saved CF oracle.

`values.json` may provide `template_files`, per-combination `files` and a
same-owner `template` override. A binary file has `{"path": "...", "binary": true}`.
Files remain below committed `input/`. `required_variants` lists same-class
dependencies retained when isolating native refusals.

Rebuild every affected combined configuration after changing a source set:

```text
python tools/coverage/combine.py --ibcmd <native-ibcmd> --platform <exact-build> --xml-version <version> --root tests/fixtures/coverage/<build> --work <new-scratch>
```

For 8.3.27.2214 also pass `--include-root tests/fixtures/coverage/8.3.27.2342`.
For 8.5.1.1529 also pass `--include-root tests/fixtures/coverage/8.5.1.1150`.
The 8.5.1.1150 combined configuration uses its own root. Multiple same-class
sets have distinct manifest identifiers; source manifest hashes keep combined
fixtures current. Explicit configuration properties and root `Ext/` sidecars
are merged with conflict checks.

Two `cross_version` sets keep an unchanged public CF and prove loading it on
a second platform. Use `tools/coverage/cross_version_set.py --help` to rebuild
them into new set and scratch directories. They remain separate from the
combined XML build: importing their XML would erase the stored cross-version
layout being tested. Their logical source variants already enter combined.

Measure all generated corpus sets with a pinned ibcmd-rs executable:

```text
python tools/coverage/measure_corpus_cases.py --ibcmd-rs <binary> --root tests/fixtures/coverage --out <new-evidence-directory>
```

This compares complete CF export bytes and audits XML compilation into rows.
It does not write SQL and does not establish SQL import/apply parity. The
special source-only and legacy probes are documented in
[`coverage-records`](../../tests/fixtures/coverage-records/README.md).

No passing baseline is removed. The old generator and the extended generator
were compared over all 95 original configuration sets from the PR; every
generated file remained byte-identical.
