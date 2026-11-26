# MAX_NATIVE_NODES and the three giant ERP УХ roles, 20260825

Status: measurement only, no code change. Answers the question left open by
`role-rights-configuration-root-20260824.md`'s "What is still refused, and
why" section -- exact node counts for the three roles it named, why the
bound is shared infrastructure rather than a Role Rights setting, and why
raising it to fit these three specifically is refused rather than attempted.

## The three roles, precisely

`src/compiler/families/native.rs`'s `MAX_NATIVE_NODES = 1_000_000` is set
from "the largest node count evidenced at the time (564,948, for a 2.27 MiB
enterprise MXL)" (`role-rights-configuration-root-20260824.md`). That doc
located three roles whose `.0` element decodes to more plaintext than any
previously-evidenced native value, but only measured their *byte* size, not
their node count. Both are now measured directly, by extracting each role's
`.0` element from `1cv8.cf` (`cf extract`, ERP УХ 3.2.12.6, unchanged since
the prior pass) and running a line-for-line Python port of
`Parser::value`/`list`/`text`/`token`/`bump_node` against the real bytes,
uncapped:

| role | uuid | plaintext bytes | nodes | max depth |
| --- | --- | ---: | ---: | ---: |
| ИспользованиеПлатежногоКалендаряУХ | `088359db-cc36-4a0b-bc66-5e30a435e5a2` | 13,651,373 | 1,164,734 | 5 |
| ЧтениеВекселей | `76fb716c-04fb-4483-a4b6-5570ed4440fa` | 14,223,665 | 1,165,170 | 7 |
| БазовыеПраваБПУХ | `ca485ab4-cdf5-4dd5-9829-ec231d26b9bf` | 16,198,940 | 1,355,230 | 7 |

All three fully round-trip under the Python port (`consumed offset == input
length` in every case) -- confirmed well-formed native-value data, not
truncated or corrupt, exceeding the current bound by 16.5%-35.5%. All three
sit at depth 5-7, far under the independent `MAX_NATIVE_DEPTH = 64` bound;
these are wide, shallow structures (a role's Rights blob is one flat
per-object list), not deep ones.

## Why this is not "just raise the constant"

`MAX_NATIVE_NODES` is not a Role Rights setting. It is `bump_node`'s bound
inside the one shared recursive-descent parser twelve independent families
construct their `NativeValue` trees through:

```
$ grep -rl "families::native" src | grep -v /native.rs
src/compiler/bodies/mxl.rs
src/mssql_dump/command_interface.rs
src/compiler/bodies/dcs.rs
src/compiler/bodies/command_interface.rs
src/compiler/bodies/template.rs
src/compiler/families/modules.rs
src/compiler/bodies/predefined.rs
src/compiler/bodies/rights.rs
src/compiler/bodies/form.rs
src/compiler/families/assets.rs
src/compiler/families/business_object.rs
src/compiler/families/commands.rs
src/compiler/families/form.rs
```

Raising the shared constant to fit these three roles raises the ceiling for
MXL spreadsheets, DCS bodies, command interfaces, templates, module bodies,
predefined data, form bodies and asset/business-object/command families at
the same time, on every corpus this project reads, evidenced or not. That
is a different -- and larger -- claim than "these three roles are
legitimate," and the density evidence argues against making it casually:

- The three roles average ~11.7-12.0 bytes per node (13,651,373 B /
  1,164,734 nodes through 16,198,940 B / 1,355,230 nodes). That is *not* a
  floor on how dense a legitimate-looking blob can be -- a run of short
  bare tokens (`0`, `1`, single-character UUIDless flags) separated by
  commas can be under 2 bytes/node. The already-accepted, independent
  `MAX_PLAIN_BYTES = 64 MiB` ceiling alone permits a blob at that density to
  reach on the order of 30-35 million nodes -- meaning `MAX_NATIVE_NODES` is
  doing real, non-redundant amplification-limiting work: it caps how much
  `Vec<NativeValue>`/`String` heap structure a single field can force the
  process to allocate, independent of how compactly that structure prints.
  A bound sized "just large enough to admit these three roles" (a ceiling
  near 1.36-1.5M) does not close that gap for the other twelve callers; a
  bound sized to the byte cap's amplification worst case would swallow the
  node bound's purpose entirely.
- No corpus-wide census of node counts across all thirteen callers exists.
  This pass adds three precise, evidenced data points for one family
  (Role Rights); it does not establish what the other twelve families'
  genuine ceilings should be, which is what a responsible global raise
  would need.

This matches the prior pass's own conclusion ("raising a security-relevant
bound needs its own evidenced ceiling ... which this pass did not
attempt") and does not overturn it: the ceiling is now sharper (exact node
counts, not just byte proxies) but still describes only one family out of
thirteen sharing the bound.

## Streaming, considered and also refused for this pass

Avoiding the bound instead of raising it -- parsing Role Rights without
materializing a full `NativeValue` tree -- would need a second code path
through the shared grammar (`value`/`list`/`text`/`token`) that yields
positions/spans instead of an owned tree, since all thirteen callers
currently receive a complete `NativeValue` and query it with tree accessors
(`as_token`, `required_list`, `exact_list`, ...). That is a parser-level
addition affecting shared infrastructure, not a `role_rights.rs`-local
change, and is out of scope for this pass for the same reason raising the
constant is: it cannot be evidenced against twelve other families' needs in
one sitting.

## Disposition

Unchanged: all three roles continue to fail closed with `native Role
Rights codec rejected data: native value exceeds its node bound`. This is
the typed refusal the corpus doctrine asks for when a fix cannot be proven
safe for every caller of shared code, not an oversight -- fixing it needs
either (a) a per-family override (Role Rights gets its own, separately
evidenced ceiling above the shared default, since the shared default
continues to protect the other twelve less-evidenced callers) or (b) the
streaming rewrite above, each a separate, larger pass with its own
corpus-wide evidence.

## Reproduction

```
ibcmd-rs cf extract 1cv8.cf <uuid>.0 <outdir>   # per role, ~15-30s each
python3 <node-counting port of native.rs's Parser, uncapped> <outdir>/unpacked.bin
```

No fixture is checked in: the exact bytes are ERP УХ 3.2.12.6 corpus data
(1.7 GiB `1cv8.cf`), not something this repository can carry, and the count
is a read-only measurement rather than a behavior this pass changes.
