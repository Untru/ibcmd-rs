# External report v5 descriptors (#553)

## Inputs and implementation

- Source: `77d81bfc89ed962708d5b716e30b199d47cc86e1`, tree
  `89640735e579ff907bfeae4f2eb164437ad3bca1`.
- Corpus: public `Untru/1c-fixtures`, pinned commit
  `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
- Native oracle: `coverage/8.5.1.1150/external/{erf-attributes,combined-erf}`,
  XML 2.21. The manifests identify native ibcmd SHA-256
  `62e72e15bb4550c4ffcf421f962c27fd1e1ddde1ba31e5bc0d7c9e0c2352dde7`.
- Local toolchain: pinned Rust 1.95.0, Windows x86-64 MSVC. Cargo used
  `CARGO_BUILD_JOBS=2` and `CARGO_PROFILE_DEV_DEBUG=line-tables-only`.
- Tested debug executable SHA-256:
  `b747b3a339e8d79fa3c1ef757505a3600b59c232f7e128378ce207f7f2ffe977`.

All 101 platform-proven external report descriptors have revision 5 and 14
header fields. Their shared fields match the corresponding 8.3.27.2342 v4
descriptors exactly. The appended member is the nil UUID in this corpus.
Report v20 already carries the corresponding AuxiliaryVariantForm member in
the internal decoder. The adapter now preserves the appended member in both
directions; a synthetic nonempty reference also tests edits and round trips.
That synthetic test is not a native measurement of a nonempty v5 reference.

## Results

The new header test was first run against the unchanged parser in an isolated
Rust harness including the actual repository modules. It failed with
`unsupported external header version 5`. The same harness passed all 11 tests
after the change. The normal root header suite subsequently passed all 10
tests (the harness additionally includes the brace reader's test).

```text
cargo test --locked -p ibcmd-rs --lib external::header::tests:: --no-default-features
10 passed, 0 failed

cargo test --locked -p ibcmd-rs --test integration external_export:: --no-default-features
8 passed, 0 failed
```

Each of the 101 corpus files was decoded from Base64, checked against its
manifest SHA-256 and exported with `cf export --source-version 2.21`. The
output's complete file map was compared with the variant's manifest-listed
native files, whose hashes were also checked. ConfigDumpInfo.xml is excluded
by the existing coverage contract. Temporary directories were removed after
each comparison.

| Set | Byte-equal | Different | Refused |
| --- | ---: | ---: | ---: |
| 8.5.1.1150 external/erf-attributes | 65 | 0 | 35 |
| 8.5.1.1150 external/combined-erf | 0 | 0 | 1 |

The 65 equal variants enter the converter-owned baseline, generated with
`tools/coverage/update_baseline.py` from the comparison results. No existing
baseline is removed or relaxed. The remaining 36 failures no longer reject
header v5: they reach the separate missing Reports output failure described
in #554 (attribute types), also covered by PR #703. This change therefore
references #553 without claiming its complete corpus acceptance.
The 65 equal names are exactly the existing 8.3.27.2342 report baseline.

The persistent gate is reproducible without an installed platform:

```text
git submodule update --init --recursive
cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features -- --nocapture
```

The author and a fresh-context critic reviewed the source and found no
correctness defect. Full local gates and the final critic assessment
remain recorded in PR #710. No fresh native 8.5 run or SQL/database mutation
was performed for these measurements.
