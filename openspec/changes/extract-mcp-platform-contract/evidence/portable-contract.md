# Bounded portable contract evidence

Base: upstream master `6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4`.
Rust 1.95.0, jobs=1, separate target and external temporary directories.
`IBCMD_TEST_KEEP_FIXTURES=1`; `IBCMD_MCP_NATIVE` unset. No private corpus,
1C native process, VM, release build or full CI was used.

| Command | Result |
| --- | --- |
| `cargo test --locked -p ibcmd-rs --test mcp_platform_contract --no-default-features` | 11 passed, 0 ignored |
| `cargo test --locked -p ibcmd-rs --lib mcp:: --no-default-features` | 9 passed, 0 ignored |
| `cargo test --locked -p ibcmd-rs --test mcp_adapter --no-default-features` | 9 passed, 0 ignored; real rmcp stdio client, no native oracle |
| `cargo check --locked -p ibcmd-rs --lib --no-default-features` | passed; root warnings retained in log, no diagnostics in owned MCP files |
| direct Rust 1.95 rustfmt and `git diff --check` | passed |
| strict offline OpenSpec validation | passed |

Independent static review requested two additions: the complete admission
matrix and same-binding content-change refusal. Both were added and re-review
found no new actionable authority/confinement/cancellation issue. This is
bounded source/contract evidence, not full merge or real-platform acceptance.

Contracts cover unsupported adapters/SQL, fixed operator binding and opt-in,
model confirmation/target overrides, relative path syntax, changed opaque
binding, same-binding content mutation, checked cumulative limits/overflow,
typed cancellation and required target+source scope. In-memory witnesses do not
stand in for native handles. Existing Unix guards and source census remain.

Full `cargo xtask ci`, cross-platform execution and real Windows/SQL adapter
negative controls remain separate gates. Existing cfg(unix) MCP tests do not
prove Windows functionality; Windows and SQL are still unsupported. No issue
closure, functional portability or publication/recovery proof is claimed.
