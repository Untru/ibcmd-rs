# Converter coverage baselines

Native inputs and oracle artifacts live in the pinned public submodule
[`../1c-fixtures`](../1c-fixtures), repository https://github.com/Untru/1c-fixtures.

This directory retains `baseline.json` and `storage-baseline.json` at the same
platform/set paths. They are converter promises and are not owned by the corpus.
Do not remove an equal variant or path to accommodate a corpus update.

```text
git submodule update --init --recursive
python tools/coverage/fixture_paths.py
cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features
```

Source probe refusal and native parity expectations remain in
`../coverage-records/source-probe-expectations.json`, keyed by stable probe ID.
