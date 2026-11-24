# External package bootstrap on the combined coverage sets

## Inputs and commands

The oracle is the native tree from [1c-fixtures at 8d138b89](https://github.com/Untru/1c-fixtures/tree/8d138b896f462c3053d16076cbabf0ffe5030c1c/coverage). Each input contains just `native/CovCombined.xml`. For each independent revision, build the CLI with Rust 1.95.0 and run both commands in a fresh directory per set.

```sh
cargo build --locked --bin ibcmd-rs --no-default-features
ibcmd-rs cf bootstrap --base-free --platform "$platform" "$set/native" "$work/out.$suffix"
ibcmd-rs cf export "$work/out.$suffix" "$work/export" --source-version "$xml"
diff -r --exclude=ConfigDumpInfo.xml "$set/native" "$work/export"
```

Use `8.3.27` and `2.20` for 8.3.27.2342, and `8.5.1.1150` and `2.21` for 8.5.1.1150. The 8.3.27.2342 build has no distinct CLI selector.

| Set | XML bytes | Native SHA-256 |
|---|---:|---|
| 8.3.27.2342 external/combined-erf | 108901 | `c2129b4035be8e1c620b1742f480b3a30927ca71a0d0997eaa5002cfe2b601ed` |
| 8.5.1.1150 external/combined-epf | 108816 | `c96bfa14caa94c86945594bd2b5faa4e909cb7b46e0b062d3eafe9d453ef8ca9` |
| 8.5.1.1150 external/combined-erf | 108984 | `435c461478bc43b7f6537578f94878978a9df9d323ac5452f05256df18d4af56` |

## Results

| Source | 8.3 ERF | 8.5 EPF | 8.5 ERF |
|---|---|---|---|
| Master `54b796f6` | missing Reports | XML 2.21 refused | XML 2.21 refused |
| [#703](https://github.com/Untru/ibcmd-rs/pull/703) `f6612b11` | equal | XML 2.21 refused | XML 2.21 refused |
| [#710](https://github.com/Untru/ibcmd-rs/pull/710) `f1b453b8` | missing Reports | XML 2.21 refused | XML 2.21 refused |
| #703 + #710 | equal | XML 2.21 refused | XML 2.21 refused |
| This change on master | missing Reports | equal | unsupported external header version 5 |
| This change + #703 + #710 | equal | equal | equal |

An equal result covers every relative path and every byte in the exported tree, apart from ConfigDumpInfo.xml. The two upstream report gaps remain listed while the required readers are pending.

## Seed provenance

The 8.5 seeds are the base64-decoded `CovAttr_xs_string` files from the pinned `epf-attributes/variants` and `erf-attributes/variants` sets. The processor SHA-256 is `12ff6ae05f9cf446e852a507de79c7fa6efb94b2be4cf894750e6efb3eae6200`. The report SHA-256 is `aa74a987661d6c4969ea88e80bf31117928cec36c4a7f1d7063e1b586c295b25`. Both packages carry a single string attribute and the 8.5 version record. The processor uses a v4 descriptor and the report uses v5.

## Native checks

On Windows, native `ibcmd --version` returned `8.3.27.2342`. In a new ru_RU file infobase, the following commands succeeded for the 8.3 report. The fresh native import and both independent packages exported to the pinned one-file tree above. The `ibcmd-rs` export of its own package matched the same tree. [Exact-build measurement in #677](https://github.com/Untru/ibcmd-rs/issues/677#issuecomment-7130953673).

```sh
ibcmd infobase create --db-path="$db" --create-database --locale=ru_RU --data="$work/create"
ibcmd config import --db-path="$db" --out="$work/native.erf" --data="$work/import" "$set/native/CovCombined.xml"
# Run for the pinned ERF, the native import, and the file built with #703.
ibcmd config export --db-path="$db" --file="$package" --data="$work/data-$case" "$work/tree-$case/CovCombined"
```

Native 8.5.1.1529 on Windows exported the EPF built by this change on master to the 108816-byte XML above. It also exported the ERF from the three-change integration to the 108984-byte native XML. Both exports contained exactly one file. The pinned 8.5.1.1150 native tree is the exact-build oracle.

## Measured Windows binaries

| Source | Git commit | SHA-256 |
|---|---|---|
| master | `54b796f6` | `cdc766ff7deca68ed7d676440cd58c479ddaf52632e02d2ca70d194bb764de09` |
| pr703 | `f6612b11` | `1420f05468b483912cdaa367a241aea4c2958971667e9bb083d633aff124a49d` |
| combined | `959d126a` | `2f6c798c445fde9a6dc6db4566e34cf690dcc55ba2428b864b52434049bfab00` |
| pr710 | `f1b453b8` | `c904ac8c0bf871b2b80e132917da9dc14b2e874260c84fa028cb160f9d5e7329` |
| change | `a09fcacb` | `bae8cc47213ce4532611cf92b5aad822f592ab869e0dce204ca27ff10de46b42` |
| change+703+710 | `8037e3a4` | `9628964dfbcbe0e8d72cefa4c6fa103e95d1b41dab38401acb0d51e2422556e2` |
