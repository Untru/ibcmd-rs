# CF fuzz harness

`cf_parse` exercises unified Format15/Format16 indexing, selected header/data
reads, raw-DEFLATE validation and bounded nested traversal. The harness caps
each input at 1 MiB and uses the same explicit resource budget as portable
regression tests.

It is an opt-in developer target, not a default workspace member or a release
dependency. After installing a Rust nightly toolchain and `cargo-fuzz`:

```text
cargo fuzz run cf_parse -- -max_len=1048576
```

Permanent minimized regressions belong in
`tests/fixtures/malformed/cf/` as reviewable clean-room `.hex` files and must be
asserted by `tests/cf_corruption.rs`.
