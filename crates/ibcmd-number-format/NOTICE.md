# OpenJDK 17 number formatter

This library translates the binary32/binary64 emission algorithm of
`jdk.internal.math.FloatingDecimal` into Rust. It is source-derived code,
not a clean-room implementation. Natural-number limb arithmetic is locally
implemented. The original copyright notice is retained in `src/lib.rs`.

Original copyright: (c) 1996, 2016, Oracle and/or its affiliates.
License: **GPL-2.0-only WITH Classpath-exception-2.0**, including the exception
for this modified library. The complete license text is in `LICENSE`.
This license applies to this library; the morph1c snapshot retains its own
Apache-2.0 license selection.

The source used for the translation is the installed Axiom JDK 17.0.16+12
`java.base/jdk/internal/math/FloatingDecimal.java` from `lib/src.zip`.
Original source SHA-256:
`3bdf29124dd5f43fa01ea82a552d2ff9db1fc39bbc1e929888bb664fe4bf8d8b`.
Corresponding public upstream:
https://github.com/openjdk/jdk17u/blob/jdk-17.0.16%2B8/src/java.base/share/classes/jdk/internal/math/FloatingDecimal.java
License source:
https://github.com/openjdk/jdk17u/blob/jdk-17.0.16%2B8/LICENSE

Modification dates: 2026-10-02 and 2026-10-03. Changes include the Rust translation, the
locally implemented unsigned limb operations, preserving the original signed
32/64-bit stopping behavior, standalone `f32/f64 -> String` APIs, and locally
implemented Java-compatible decimal/hexadecimal lexical parsing with direct
IEEE binary32/binary64 rounding. The lexical parser is not translated from
OpenJDK source; it is distributed under this library's license.
The formatter has no dependencies beyond the Rust standard library.
Its tests compare actual installed JDK outputs, including subnormals,
rounding boundaries, signed zero and nonfinite values.

The binary archive distributes the complete modified library source
(`Cargo.toml`, `src/lib.rs`, `src/parse.rs`), this notice and the license under
`third-party/ibcmd-number-format/`. SBOM records the package's actual license.
