# Issue 23 V8 container layer

Date: 2026-06-30

Reference checked: `https://github.com/ava57r/v8unpack-rs` at
`f32bff20e77a0d7a711bd454b576e40b1c69875a`.

## Problem

`ibcmd-rs` already contains a minimal V8 container implementation inside
`src/module_blob.rs`. It is used for module body blobs shaped as
`deflate(V8File(info,text))` and for a few focused tests. The implementation
knows the core container layout:

- file header: 16 bytes;
- magic/end marker: `0x7fffffff`;
- default page size: 512;
- block header: 31 bytes, formatted as hex sizes;
- element address table entries: 12 bytes;
- element names stored after a 20-byte prefix as UTF-16LE.

This logic is currently private to `module_blob.rs`, duplicated in test helper
code, and intentionally limited. In particular, `read_block_payload` rejects
multi-page V8 blocks with `multi-page V8 blocks are not supported yet`.

The `v8unpack-rs` project provides a useful independent reference for this
container layer, including `V8File`, `V8Elem`, recursive nested containers,
pack/unpack, and block-chain reading. It should be used as a behavior reference,
not copied as executable code or added as a production dependency.

## Goal

Create a shared, tested V8 container module that can parse and build the
container format needed by Config/ConfigSave blobs and future `.cf`/nested
container work.

The first milestone should preserve all existing module blob behavior while
moving the low-level container handling out of `module_blob.rs`.

## Scope

Primary files:

- `src/v8_container.rs` or equivalent new module;
- `src/lib.rs`;
- `src/module_blob.rs`;
- focused tests in the new module and existing module blob tests.

Reference files to study before implementation:

- `v8unpack4rs/src/container.rs`;
- `v8unpack4rs/src/parser/single.rs`;
- `v8unpack4rs/src/builder/mod.rs`.

Implementation scope:

- introduce a small internal API for parsing a V8 container from bytes;
- expose element name, raw header bytes, and raw data bytes;
- support building a container from named elements while preserving existing
  `module_blob` output shape;
- support block headers and address table validation with explicit diagnostics;
- support reading chained multi-page blocks or explicitly prove they are not
  needed for the current target data and keep the unsupported boundary tested;
- replace module blob's private container parser/builder with the shared module;
- remove or consolidate duplicated V8 container test helpers where practical.

## Non-goals

- Do not add `v8unpack-rs` as a runtime dependency.
- Do not copy code directly from `v8unpack-rs`; use it only as a reference for
  independently implemented behavior.
- Do not implement full empty-infobase bootstrap import.
- Do not redesign metadata/form/template payload parsers in this issue.
- Do not claim full `.cf` compatibility unless tested with real `.cf` fixtures.

## Acceptance

- Existing `pack_module_blob_bytes` and `unpack_module_blob_text` behavior is
  unchanged for current tests.
- A new focused test parses a synthetic V8 container with at least two elements
  and verifies names/data.
- A new focused test builds a container and parses it back with byte-identical
  element data.
- A module blob round-trip still succeeds through the new shared API.
- Multi-page block behavior is covered by a test: either supported by parsing a
  chained synthetic block, or rejected with a precise expected error.
- No broad metadata XML, form XML, rights, template, or SQL staging behavior is
  changed.

## Verification

Run:

```powershell
cargo test module_blob
cargo test v8_container
cargo fmt --check
git diff --check
```

If the implementation touches broader dump/staging paths, also run the relevant
targeted tests for those modules before closing the issue.

## Follow-up candidates

- Add optional `.cf` fixture-based parse/build smoke tests.
- Add a debug command that lists V8 container elements from a blob or file.
- Use the shared module for other binary-body families that currently hand-roll
  V8 container assumptions.
- Evaluate whether selected Config/ConfigSave rows can be inspected through the
  shared parser during source parity audits.
