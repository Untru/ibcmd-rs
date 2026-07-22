# EventSubscription native layout evidence for 8.3.27.1989

Status: implemented as the `event-subscription-v1-crlf-no-bom` experimental
platform layout. The codec is standalone and does not require an installed 1C
platform, EDT, JVM, or database access.

## Retained sources

- Primary XCF 2.20 source:
  `E:\ibcmd_lab\ut\ut_ibcmd\EventSubscriptions\ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных.xml`.
- Inflated primary row:
  `E:\ibcmd_lab\batch130_134_fix_full_20260718\Config_inflated\a64b15fa-fc34-43fe-a366-d27c0f1c3df2__part0.txt`.
- Inflated row length: 654 bytes; SHA-256:
  `fbf8fb56ea5127f55b58dc158bb1ec1c919bb90ebb5c41a9960451101aed7182`.
- The same retained snapshot contains 101 independently paired
  EventSubscription XCF/native rows. They cover 15 exact event spellings:
  `BeforeDelete`, `BeforeWrite`, `FillCheckProcessing`, `Filling`,
  `FormGetProcessing`, the five send/receive node events, `OnSetNewNumber`,
  `OnWrite`, `Posting`, `PresentationFieldsGetProcessing`, and
  `PresentationGetProcessing`.

The implementation reads these retained artifacts only as test evidence. It
does not start 1C or modify Config/ConfigSave.

## Exact native grammar

The primary row is an exact three-field root:

```text
{1, EventSubscriptionObject, 0}
```

`EventSubscriptionObject` has exactly six fields: discriminator `1`, common
metadata header, a non-empty `"Pattern"` collection of TypeIds, the exact
localized native event token, CommonModule UUID, and method name. TypeIds and
the handler module are resolved from readable XCF names only through the
validated canonical graph. Missing, nil, duplicated, unknown, reordered, or
extra native fields fail closed.

The native event token includes an English stable prefix and a retained
Russian suffix. All 15 mappings are explicit and reversible; a future event
does not inherit a guessed suffix. Native TypeIds do not preserve the XCF
`v8:Type` versus `v8:TypeSet` spelling, so native IR-to-XCF requires an exact
caller-provided readable mapping for each TypeId.

## Version boundary

XCF 2.20 and 2.21 have explicit dialect codecs over this one evidenced native
layout. Platform build, storage profile, and XML dialect remain independent
axes. A future platform must select a separately evidenced layout constant.
