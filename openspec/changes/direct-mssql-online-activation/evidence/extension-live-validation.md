# Extension online activation: live 8.3.27 validation

- Platform client/server: `8.3.27.2214`.
- Cluster: `24c580ef-d5de-4b78-b204-b94b64eb2fae`, server port `2541`, RAS port `2545`.
- Disposable MSSQL clone: `ibcmd_rs_activation_ext_20260829`.
- Extension: `_ДемоРасширение`.
- Changed existing client common-module body:
  `cd9aecaa-e480-481c-8c77-ff6600837681.0`.
- Publication executable: `ibcmd-rs` only; native `ibcmd` was not invoked in
  these live passes.

The direct publisher switched the registry/CAS root from
`47ceaeb25da80aa16dc57f8f6fa0b19f7ce6f7b3` to
`e4283fbf08d6e3b0d72a72a0153032282ad1173b` while PID `66004` remained an
active 1C client session. That session continued to execute its loaded
generation:

```text
29.08.2026 13:05:43|after-extension-direct
```

After it was closed, a fresh client initially received the old server-cached
generation and then, after the 8.3.27 polling interval without any cluster or
working-process restart, executed the published generation:

```text
29.08.2026 13:06:39|after-extension-online-v2
```

Thus online publication is asynchronous. Existing sessions retain their
loaded extension generation; fresh sessions converge to the published
generation after platform polling. The exclusive mode must separately reject
other database sessions before performing the same authoritative switch.
