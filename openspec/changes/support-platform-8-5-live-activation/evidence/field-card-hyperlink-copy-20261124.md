# InputField and LabelField on 8.5.1.1529

The oracle was native ibcmd 8.5.1.1529, SHA-256
`4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20`.
Fixtures came from `Untru/1c-fixtures@8d138b89`, under
`coverage/8.5.1.1529/{corpus-modern-details,corpus-hybrid-interface}/native`.

For each set, retain the language and the four CommonForms below, removing
the other CommonForm entries from Configuration.xml. Make a second copy
with just the property under test omitted from each Form.xml. For each of
these four configurations use a fresh directory for both DB and data.

```text
ibcmd infobase create --db-path=DB --data=DATA --locale=ru_RU --import=SRC
ibcmd infobase config save --db-path=DB --data=DATA native.cf
ibcmd infobase config export --db-path=DB --data=DATA native-xml
```

Each inflated form body (the CommonForm UUID followed by `.0`) differed
from its control by exactly one ASCII digit. Both CovModern and CovHybrid
gave these values.

| Form suffix | XML value | Record, appended member (zero-based) | Omitted → explicit |
| --- | --- | --- | --- |
| InputFieldCellHyperlinkRepresentation | DontShow | item 48, 13 | 0 → 2 |
| LabelFieldCellHyperlinkRepresentation | DontShow | item 48, 13 | 0 → 2 |
| InputFieldWidthInCard | Full | item 48, 8 | 0 → 1 |
| LabelFieldUseCopy | false | bag 12, 0 | 2 → 0 |

An additional native InputField with several properties wrote ShowTitleInCard,
WidthInCard, AutoWidthInTable, CellHyperlinkRepresentation and
CellHyperlinkDisplayVariant in that order.

The following checks used ibcmd-rs based on `54b796f6`, with the field
changes applied (Windows debug executable SHA-256
`cf7e02a7349db8739c1ae3ad5ffe962c0d9ec7220d1ebb56b59b3f83e3d77b3e`).
Use the measured sources above. For the Hybrid writer checks change only
Configuration.xml's InterfaceCompatibilityMode from TaxiEnableVersion8_5
to Taxi because #678 blocks the original root. Keep the native Hybrid
exports and bodies from the original interface mode as the references.

```text
ibcmd-rs cf bootstrap --base-free --platform 8.5.1.1529 SRC ours.cf
ibcmd-rs cf export ours.cf cf-xml --source-version 2.21
ibcmd-rs infobase config import --no-verify --db-path=EMPTY_FILE_DB SRC
ibcmd-rs infobase config export --db-path=EMPTY_FILE_DB db-xml
```

The file route started from the decompressed tests/fixtures/file-infobase/empty.1CD.gz.
All eight Form.xml files on each route matched the respective native export
byte for byte. All eight inflated form bodies in ours.cf matched native.cf.
