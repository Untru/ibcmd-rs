# Report, DataProcessor, Enum and SettingsStorage rows on 8.3.27

Status: bounded evidence for the standalone compiler profile `platform-8.3.27.1989`.

The evidence was obtained by comparing XCF sources with inflated `Config` rows from the repeatable SQL-storage laboratory described in `docs/ssl-lab-2026-06-25.md`. No 1C platform, EDT runtime or JVM is used by the implemented codec.

## Verified root layouts

| Family | Root fields | Body discriminator / fields | Ordered collections |
| --- | ---: | --- | --- |
| Report | 8 | `19` / 18 | Template, Attribute, Form, TabularSection, Command |
| DataProcessor | 8 | `17` / 12 | TabularSection, Template, Command, Form, Attribute |
| Enum | 7 | `20` / 21 | Form, Template, reserved-empty, EnumValue |
| SettingsStorage | 5 | `2` / 8 | Template-empty, Form |

The collection UUIDs, generated TypeId/ValueId slots, header positions and child ordering are encoded as exact constants in `src/compiler/families/utility.rs`. The native decoder independently checks the same inventory and rejects extra fields, unknown collection markers, wrong counts, duplicate identities, malformed headers and future layouts.

## Embedded objects

- Report and DataProcessor direct `Attribute` rows use the evidenced 23-field payload.
- Their `TabularSection` rows contain two ordered generated-type pairs and a nested Attribute collection with a family-specific marker.
- `Command` uses the fixed evidenced command ValueId `078a6af8-d22c-4248-9c33-7e90075a3d2c`.
- Enum values use the exact header wrapper; the optional `Color` source property is accepted only as `auto`.
- Enum `Order`/`Ref` and tabular `LineNumber` standard attributes use the exact 25-property native descriptor. The decoder compares the full descriptor, not only its marker.

## Representative source/native pairs

- Report: `АнализВерсийОбъектов` (minimal), `АнализИсполненияАссортимента` (attributes/templates), `НаличиеСчетовФактур` (tabular section).
- DataProcessor: `АктивныеЗаказыСУЗ` (forms), `БлокировкаРаботыПользователей` (attributes/forms), `ВиртуальнаяАгрегацияУпаковокИСМП` (tabular section).
- Enum: `ВариантыВыводаМесяцаВДатеДокумента` (two values), `ВариантыДействийПоРасхождениямВАктеПослеПриемки` (form reference).
- SettingsStorage: `БуферыОбменаНовостей` and `ХранилищеВариантовОтчетов`.

## Compatibility boundary

The layout is enabled only when all independent coordinates match:

- XML dialect `2.20` or `2.21`;
- platform build `8.3.27.1989`;
- storage profile `storage:mssql-config-configsave`;
- no inferred compatibility mode or container revision;
- the family-specific layout constant listed above.

Any missing or future coordinate fails closed. A new platform release therefore adds a profile/evidence entry and, only if necessary, a new layout implementation without changing existing conversions.
