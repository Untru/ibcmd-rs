# Configuration-body evidence for platform 8.3.27.1989

This note records the structural evidence and clean-room generation contract
behind `bootstrap.configuration.layout =
configuration-v68-seven-sections-v1`.

The production codec is standalone Rust. It does not invoke a 1C executable,
Designer, `ibcmd`, EDT, a JVM, SQL Server, or any other external process.
Database inspection described below was a one-time read-only research step and
is not part of build, test, CLI, or release execution.

## Evidence cohort

On 2026-07-22 and 2026-07-23 two Configuration rows from local test
infobases were inspected read-only with the existing bounded native parser:

- a layout-67 row with 60 property fields and 24 first-section families;
- a layout-68 row with 61 property fields, final field `1`, and 25
  first-section families.

Only the second cohort is enabled by the 8.3.27.1989 profile. Layout 67 is
useful comparison evidence but cannot be selected accidentally by this codec.
No application strings or native row bytes are retained in the repository.

Both rows confirmed the same outer contract:

```text
{2,{Configuration UUID},7,<section 1>...<section 7>,{{0,"",""}}}
```

The exact section order and wrappers for layout 68 are:

| Section | Class ID | Family slots | Wrapper before the family count |
|---:|---|---:|---|
| 1 | `9cd510cd-abfc-11d4-9434-004095e12fc7` | 25 | `{1,{68,...}` |
| 2 | `9fcd25a0-4822-11d4-9414-008048da11f9` | 15 | `{6,{1,{{1,0,ObjectId},nil}` |
| 3 | `e3687481-0a87-462c-a166-9f34594f9bba` | 2 | `{1,{0,{1,0,ObjectId}}` |
| 4 | `9de14907-ec23-4a07-96f0-85521cb6b53b` | 2 | `{1,{{1,0,ObjectId}}` |
| 5 | `51f2d5d8-ea4d-4064-8892-82951750031e` | 2 | `{1,{0,{1,0,ObjectId}}` |
| 6 | `e68182ea-4237-4383-967f-90c1e3370bc7` | 1 | `{1,{{1,0,ObjectId}}` |
| 7 | `fb282519-d103-4dd3-bc12-cb271d631dfc` | 1 | `{1,{{1,0,ObjectId}}` |

Each section contains exactly one non-zero internal ObjectId. Observed IDs
were configuration-specific and had no stable relation to the outer UUID.
The standalone writer therefore uses a documented project policy instead of
copying or guessing an observed value: RFC 9562 UUIDv8 derived from a
domain-separated SHA-256 digest of profile ID, Configuration UUID, and section
class ID. Repeated builds are byte-identical and different profiles/classes
cannot share the same derivation domain.

## Family directory

Representative primary rows were classified with the existing strict native
metadata discriminators. The resulting slot mapping is profile-local:

| Section | Slot class ID | Canonical family |
|---:|---|---|
| 1 | `09736b02-9cac-4e3f-b4f7-d3e9576ab948` | Role |
| 1 | `0c89c792-16c3-11d5-b96b-0050bae0a95d` | CommonTemplate |
| 1 | `0fe48980-252d-11d6-a3c7-0050bae0a776` | CommonModule |
| 1 | `0fffc09c-8f4c-47cc-b41c-8d5c5a221d79` | HTTPService |
| 1 | `11bdaf85-d5ad-4d91-bb24-aa0eee139052` | ScheduledJob |
| 1 | `15794563-ccec-41f6-a83c-ec5f7b9a5bc1` | CommonAttribute |
| 1 | `24c43748-c938-45d0-8d14-01424a72b11e` | SessionParameter |
| 1 | `30d554db-541e-4f62-8970-a1c6dcfeb2bc` | FunctionalOptionsParameter |
| 1 | `37f2fa9a-b276-11d4-9435-004095e12fc7` | Subsystem |
| 1 | `39bddf6a-0c3c-452b-921c-d99cfa1c2f1b` | Interface |
| 1 | `3e5404af-6ef8-4c73-ad11-91bd2dfac4c8` | Style |
| 1 | `3e7bfcc0-067d-11d6-a3c7-0050bae0a776` | FilterCriterion |
| 1 | `46b4cd97-fd13-4eaa-aba2-3bddd7699218` | SettingsStorage |
| 1 | `4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc` | EventSubscription |
| 1 | `58848766-36ea-4076-8800-e91eb49590d7` | StyleItem |
| 1 | `6e6dc072-b7ac-41e7-8f88-278d25b6da2a` | Bot |
| 1 | `7dcd43d9-aca5-4926-b549-1842e6a4e8cf` | CommonPicture |
| 1 | `857c4a91-e5f4-4fac-86ec-787626f1c108` | ExchangePlan |
| 1 | `8657032e-7740-4e1d-a3ba-5dd6e8afb78f` | WebService |
| 1 | `9cd510ce-abfc-11d4-9434-004095e12fc7` | Language |
| 1 | `a7641777-7813-45c6-96ef-9d51587a6ac6` | Reserved; must remain empty |
| 1 | `af547940-3268-434f-a3e7-e47d6d2638c3` | FunctionalOption |
| 1 | `c045099e-13b9-4fb6-9d50-fca00202971e` | DefinedType |
| 1 | `cc9df798-7c94-4616-97d2-7aa0b7bc515e` | XDTOPackage |
| 1 | `d26096fb-7a5d-4df9-af63-47d04771fa9b` | WSReference |
| 2 | `0195e80c-b157-11d4-9435-004095e12fc7` | Constant |
| 2 | `061d872a-5787-460e-95ac-ed74ea3a3e84` | Document |
| 2 | `07ee8426-87f1-11d5-b99c-0050bae0a95d` | CommonForm |
| 2 | `13134201-f60b-11d5-a3c7-0050bae0a776` | InformationRegister |
| 2 | `1c57eabe-7349-44b3-b1de-ebfeab67b47d` | CommandGroup |
| 2 | `2f1a5187-fb0e-4b05-9489-dc5dd6412348` | CommonCommand |
| 2 | `36a8e346-9aaa-4af9-bdbd-83be3c177977` | DocumentNumerator |
| 2 | `4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d` | DocumentJournal |
| 2 | `631b75a0-29e2-11d6-a3c7-0050bae0a776` | Report |
| 2 | `82a1b659-b220-4d94-a9bd-14d757b95a48` | ChartOfCharacteristicTypes |
| 2 | `b64d9a40-1642-11d6-a3c7-0050bae0a776` | AccumulationRegister |
| 2 | `bc587f20-35d9-11d6-a3c7-0050bae0a776` | Sequence |
| 2 | `bf845118-327b-4682-b5c6-285d2a0eb296` | DataProcessor |
| 2 | `cf4abea6-37b2-11d4-940f-008048da11f9` | Catalog |
| 2 | `f6a80749-5ad7-400b-8519-39dc5dff2542` | Enum |
| 3 | `238e7e88-3c5f-48b2-8a3b-81ebbecb20ed` | ChartOfAccounts |
| 3 | `2deed9b8-0056-4ffe-a473-c20a6c32a0bc` | AccountingRegister |
| 4 | `30b100d6-b29f-47ac-aec7-cb8ca8a54767` | ChartOfCalculationTypes |
| 4 | `f2de87a8-64e5-45eb-a22d-b3aedab050e7` | CalculationRegister |
| 5 | `3e63355c-1378-4953-be9b-1deb5fb6bec5` | Task |
| 5 | `fcd3404e-1523-48ce-9bc0-ecdb822684a1` | BusinessProcess |
| 6 | `5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5` | ExternalDataSource |
| 7 | `bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d` | IntegrationService |

Only top-level canonical objects enter these lists. Owned descendants remain
inside their owner row. Every listed UUID must resolve to a primary entry in
the validated bootstrap graph; unknown top-level families, wrong reference
kinds, duplicate role/mobile values, and future layout IDs fail closed.

## Property layout and verification

The layout-68 writer emits exactly 61 property fields. It supports the typed
Configuration header, localized information, run/script modes, vendor/version
data, compatibility selectors, use purpose, default Style/Language/Role and
SettingsStorage references, and the 38-entry mobile capability set. Remaining
profile-required tables use explicit clean-room defaults derived from the two
structural cohorts. Raw or opaque native fragments are never accepted.

`compiler::root` tests independently split the generated native text and
assert:

- the exact outer shape, footer, section classes/wrappers, and 48 slots;
- a 61-field layout-68 projection whose final field is `1`;
- seven unique UUIDv8 internal IDs;
- every top-level canonical UUID appears exactly once and no extra UUID is
  listed;
- deterministic compressed bytes across repeated builds;
- fail-closed behavior for unknown families and reference-kind mismatches.

The target profile remains `experimental`: these offline structural and
semantic checks do not claim a native-platform acceptance run. Such an oracle
may add evidence later, but it is neither required nor permitted in the
standalone runtime or release artifact.
