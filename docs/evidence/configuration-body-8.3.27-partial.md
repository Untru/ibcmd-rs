# Partial Configuration-body evidence for platform 8.3.27.1989

This note records evidence that is useful for a future base-free
`Configuration` metadata compiler but is not sufficient to enable one.

On 2026-07-22 a read-only query inspected the row named by configuration UUID
`61ee2494-c14a-4992-8c93-8e78b20bea27` in a local test infobase. The query did
not invoke a 1C executable and did not modify the database.

- compressed bytes: 288,119;
- compressed SHA-256:
  `d436dcfd5d75005b7f890973a608dd71ee23095f043b57224d7abbcd21ca21c7`;
- decompressed characters: 503,495;
- outer fields: discriminator `2`, `{configuration UUID}`, count `7`, seven
  contained objects, and a footer;
- the footer recognized by the existing strict reader is `{{0,"",""}}`.

The seven observed contained-object class IDs, in native order, were:

1. `9cd510cd-abfc-11d4-9434-004095e12fc7`
2. `9fcd25a0-4822-11d4-9414-008048da11f9`
3. `e3687481-0a87-462c-a166-9f34594f9bba`
4. `9de14907-ec23-4a07-96f0-85521cb6b53b`
5. `51f2d5d8-ea4d-4064-8892-82951750031e`
6. `e68182ea-4237-4383-967f-90c1e3370bc7`
7. `fb282519-d103-4dd3-bc12-cb271d631dfc`

Their immediate payloads declared respectively 25, 6, 2, 2, 2, 1, and 1
collection records. Each contained object also had a configuration-specific
non-zero object ID distinct from the outer configuration UUID.

This observation does **not** establish the meaning and exact defaults of all
property slots, the family UUID-to-kind mapping, the policy for creating the
seven object IDs, or the accepted body layout cohort. The generic XCF adapter
also retains most Configuration properties as same-profile opaque XML rather
than a cross-profile typed projection. Consequently the standalone compiler
must continue returning an explicit `Unsupported` outcome for this row. A
profile may enable synthesis only after those fields and a native
load/export roundtrip are independently evidenced.
