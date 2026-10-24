---
name: coverage-case
description: Turn a mismatch found on a typical configuration (ERP, Бухгалтерия, БСП, УТ, ДО) or a lab parity run into a clean-room coverage variant that the offline test shows red, then fix it and move the variant into the baseline. Use when a native comparison reports a differing or refused file, before changing any decoder or writer for it.
---

# From a mismatch to a coverage case

A typical configuration finds a problem; the coverage sets keep it from coming
back. The fix is never made against the typical configuration's object: it
is made against a generated, platform-proven variant that reproduces the same
rule (`tests/fixtures/coverage/README.md`).

## 1. Pin the symptom

Record, for the PR:
- the configuration, the platform build and the XML version;
- the differing or refused path (`Catalogs/X.xml`, `…/Form.xml`);
- the first differing lines, native against ours (`diff` of the two files);
- for a refusal, the error code and message of `ibcmd-rs`.

## 2. Find the rule, not the object

Read the native and our file side by side and name what differs: one property
value, an element that is missing or extra, an order, a record layout. Then
classify it:

| Kind | Example | Where the case goes |
|---|---|---|
| A scalar property value | `<AutoUse>Use</AutoUse>` written as `DontUse` | add the value to `input/values.json` of the class set |
| Values of two properties that depend on each other | code length 0 needs input by string without the code | a combined variant of both values |
| A child object or a structure | an attribute type, a form element, a template, a command | a donor-based variant (`external_set.py --attributes-donor`) or a child template in the class set |
| A class without a set yet | registers, documents | a new set (`new_set.py`); mandatory properties come from the platform's refusal message |
| A source kind | an extension, an `.epf`/`.erf` | `extension_set.py` / `external_set.py` |
| A legacy layout the current platform cannot write | a form of revision 2 written by an old platform | a minimized, sanitized raw fragment marked `legacy`, plus the platform version that wrote it |

A value seen in the typical configuration may enter `values.json` with the
source `observed:<configuration>`. Its objects, modules and texts never do.

## 3. Prove the case on the platform

Rebuild the set and run it through the native platform of the build that showed
the mismatch:

```text
python3 tools/coverage/generate.py <set> <scratch>/tree
python3 tools/coverage/native_round_trip.py --ibcmd <ibcmd> --platform <build> \
  --xml-version <version> --set <set> --tree <scratch>/tree --work <scratch>/rt
python3 tools/coverage/combine.py --ibcmd <ibcmd> --platform <build> \
  --xml-version <version> --root tests/fixtures/coverage/<build> --work <scratch>/combined
```

The variant must come out `platform_proven`. If the platform refuses it, its
message says which other property the value needs: add that as a combined
variant instead.

## 4. See it red

```text
cargo test --locked -p ibcmd-rs --test coverage_corpus --no-default-features -- --nocapture
```

The new variant must be reported `different` or `refused` with the same
symptom as in step 1. If it comes out equal, the case does not reproduce the
mismatch: go back to step 2.

## 5. Fix, then hold it

- Fix the decoder or writer by the rule found in step 2, under the Traps of
  `AGENTS.md`: schema-owned slots, no per-object cases, fail closed.
- Run the coverage test again; the variant is now `equal`, and nothing in any
  baseline regressed.
- Add it to the baseline:
  `cargo test … -- --nocapture | python3 tools/coverage/update_baseline.py`.
- Commit the set, the combined set and the fix together. The PR names the
  typical-configuration symptom, the variant that reproduces it and the rule.

## 6. Close the loop

Rerun the typical configuration (lab runner, `gh workflow run lab-parity.yml`).
The whole class of mismatches should be gone, not only the file from step 1.
A file that still differs is a new case: start again from step 1.
