# Registers, recalculations, and charts — platform 8.3.27.1989

Status: implemented as eight independent experimental layouts for the
standalone compiler. Compilation and tests do not start 1C, EDT, a JVM, or
read an existing row as a base artifact.

## Retained evidence

The exact native shells were derived from the retained 8.3.27.1989 corpus at
`E:\ibcmd_lab\batch133_register_roots_full_20260718`. Each source identity was
matched to its inflated `Config/<uuid>` row. The SHA-256 values below cover the
inflated bytes used during layout analysis.

| Family / representative source | UUID | Inflated bytes | SHA-256 |
| --- | --- | ---: | --- |
| InformationRegister `_ДемоГрафикиРаботы` | `6669f240-717a-464b-b0c0-5ce915ef0e1f` | 9,829 | `bc301d62915cfe47fd202378e84fb01e79b8c3d8e330e1f3a0331f1b7285542f` |
| AccumulationRegister `_ДемоОборотыПоСчетамНаОплату` | `6707f7f5-4a6c-4d8b-84cf-483768cb71b9` | 9,881 | `677d69a4b8957cd4ed83a88b47af36cd189585d523f7a15934e08af9dbc8f9dc` |
| AccountingRegister `_ДемоЖурналПроводокБухгалтерскогоУчета` | `a61abe1c-de4e-43cc-bee3-aeb81b5dfa0c` | 26,888 | `cf7df83f304680364eefbaccbcdc12cc390a0589d6f5cb1e8165d51fade7c2e9` |
| CalculationRegister `_ДемоОсновныеНачисления` | `a6756af4-964f-400d-97d4-0bd3f5916693` | 24,449 | `45c0fe5d4c86400bad23c0abfe8b78a8e91582c231a9f5b504ef64e8c416abe2` |
| Recalculation `ПерерасчетОсновныхНачислений` | `8215bfd6-dfdd-4bea-b079-3812893a1c4e` | 1,152 | `949298c052678bc7f2314f0c319fc1a843798c8aed4aef095dcf33df26524554` |
| ChartOfCharacteristicTypes `_ДемоВидыСубконто` | `b850d595-0fab-4f29-9eb2-334fe98c8adf` | 19,728 | `f4302ef7d6b45b0200d4f014b271f2c4b69ab6a18190a6d505c07e0f39889124` |
| ChartOfAccounts `_ДемоОсновной` | `d52cf615-a846-44fb-9b13-61084d0a3c77` | 31,707 | `5efe2c6501195c6bb6fcc3f0211eae215b9093402efb63e136761043a4dd5806` |
| ChartOfCalculationTypes `_ДемоОсновныеНачисления` | `a1c76fb2-6237-41a1-8b2f-66d8c1ed2fd9` | 33,665 | `05d8df590a915554f86502ac493959b96308a904c000c12db07a095650574ec5` |

The strict read-only parsers in `src/mssql_dump/mod.rs` are a second executable
description of the owner tuples, generated-type slots, standard fields, and
collection UUIDs. The standalone compiler shares no execution path with those
parsers.

## Exact native roots

| Family | Root fields | Body discriminator / fields | Generated pairs | Root collections |
| --- | ---: | --- | ---: | ---: |
| InformationRegister | 9 | `33` / 39 | 7 | 6 |
| AccumulationRegister | 9 | `28` / 26 | 6 | 6 |
| AccountingRegister | 9 | `21` / 30 | 7 | 6 |
| CalculationRegister | 10 | `21` / 33 | 7 | 7 |
| Recalculation | 4 | `4` / 9 | 3 | one Dimension collection |
| ChartOfCharacteristicTypes | 8 | `34` / 59 | 6 | 5 |
| ChartOfAccounts | 10 | `32` / 57 | 7 | 7 |
| ChartOfCalculationTypes | 8 | `35` / 63 | 11 | 5 |

The compiler emits the exact evidenced standard-attribute and standard-table
descriptors. Generated TypeId/ValueId pairs are retained from normalized XCF
for registers and Recalculation. Full chart sources may omit `InternalInfo`;
their pairs are then generated as deterministic, domain-separated UUIDv8
values from the object UUID, family, generated kind, and role.

`CalculationRegister` retains its ordered Recalculation references.
`Recalculation` validates that every embedded Dimension points to a Dimension
owned by its CalculationRegister and emits the same target in both native
reference positions. Semantic ownership and physical storage are independent:
an owned Recalculation declares its own explicit storage route, while ordinary
embedded fields declare none.

## Profile and support boundary

The platform profile selects one constant per family. No layout is inferred
from XML dialect, compatibility mode, or a nearby platform version. XCF 2.20
and 2.21 are accepted only through the strict typed codecs.

This first supported cohort covers normalized full root properties, generated
identities, form references, CalculationRegister/Recalculation links, and
Recalculation Dimensions. Non-empty register Dimension/Resource/Attribute
collections, templates, customized standard-field bags, and compact source
files that omit required default semantics are rejected instead of guessed.
Those cohorts require their own evidence and follow-up layout support.

Portable tests cover every family through XCF decode, cross-dialect 2.20 to
2.21 encode/decode, deterministic native compilation, strict native inventory
decode, generated identity uniqueness, Recalculation ownership, and rejection
of unknown XML properties or extra native root fields.
