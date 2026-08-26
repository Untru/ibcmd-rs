# Ordinary forms: the declared form-type discriminator, 20260826

Base `911e86e`. Corpus: all eight stand configurations
(`$D = /Users/untru/Documents/ChatGPT/ibcmd-stand`).

## The refusal this closes

`cf export` of ERP УХ 3.2.12.6 refused nine form bodies with

```
failed to parse form body from source asset <файл>:
Form body blob is not valid UTF-8: invalid utf-8 sequence of 1 bytes from index 0
```

The reader was told to treat every form body as the braced UTF-8 managed-form
container. Nine bodies are not text at all.

## What the platform actually writes

The native tree writes those nine forms as `Ext/Form.bin`, not `Ext/Form.xml`,
and their declaration carries `<FormType>Ordinary</FormType>`:

```
$D/cap/uh-r1/src/Catalogs/Варианты/Forms/ФормаВыбора.xml
        <FormType>Ordinary</FormType>
$D/cap/uh-r1/src/Catalogs/Варианты/Forms/ФормаВыбора/Ext/   ->   only Form.bin
```

`Form.bin` is the stored body inflated, byte for byte, with nothing added and
nothing rendered. Verified on all nine (`cf extract <uuid>.0` -> `unpacked.bin`
vs the native `Form.bin`, sha256):

| form | bytes | sha256 equal |
|---|---:|---|
| `Catalogs/Варианты/Forms/ФормаВыбора` | 11 038 | yes |
| `Catalogs/Варианты/Forms/ФормаСписка` | 7 895 | yes |
| `Catalogs/Варианты/Forms/ФормаЭлемента` | 10 799 | yes |
| `Catalogs/ПараметрическоеУсловие/Forms/ФормаВыбора` | 7 362 | yes |
| `Catalogs/ЧувствительностьПоказателя/Forms/ФормаСписка` | 11 869 | yes |
| `Catalogs/ЧувствительностьПоказателя/Forms/ФормаЭлемента` | 28 011 | yes |
| `CommonForms/ФормаРедактированияСпискаЗначений` | 14 602 | yes |
| `Reports/ПланПоказателей/Forms/ФормаОтчета` | 28 180 | yes |
| `Reports/СинхронизацияНСИ/Forms/ФормаВарианта` | 139 772 | yes |

For the first of them:

```
sha256(native .../ФормаВыбора/Ext/Form.bin)
  = 1826f35b445c967e9c672b53a645732e770a30ca1aaaa63c412bdc6311d1910e
sha256(inflate(cf entry ed103b94-8ed1-443a-a7ea-5a2eb7fc6fbc.0))
  = 1826f35b445c967e9c672b53a645732e770a30ca1aaaa63c412bdc6311d1910e
```

Nothing else is written under an ordinary form's directory: no `Form.xml`, no
`Ext/Form/Module.bsl`, no `Items/`. The whole form is that one file.

## The discriminator the record declares

The form's own metadata record is a `{13,<header>,<a>,<b>,<usePurposes>}`
block. Slot 3 (`<b>`) is the form type. Ordinary
`Catalogs/Варианты/Forms/ФормаВыбора`:

```
{13,
{2,
{1,0,ed103b94-8ed1-443a-a7ea-5a2eb7fc6fbc},"ФормаВыбора",
{1,"ru","Форма выбора"},"",0,1,32e087ab-1491-49b6-aba7-43571b41ac2b,3,
00000000-0000-0000-0000-000000000000},0,0,
{2,{"#",1708fdaa-cbce-4289-b373-07a5a74bee91,1},
   {"#",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}}
```

managed `Catalogs/Валюты/Forms/ФормаСписка`:

```
{13,
{3,
{1,0,5f91b00f-d8fc-4d63-8486-66339357ab22},"ФормаСписка",
{2,"ru","Форма списка","en","List form"},"",0,0,
00000000-0000-0000-0000-000000000000,0},0,1,
{2,{"#",1708fdaa-cbce-4289-b373-07a5a74bee91,1},
   {"#",1708fdaa-cbce-4289-b373-07a5a74bee91,2}}}
```

Slot 2 is `IncludeHelpInContents`, which the reader already read. Slot 3 was
never read at all -- `format_form_source_xml` printed the string
`<FormType>Managed</FormType>` unconditionally.

### Census

Every form of all eight corpora, its slot-3 value against the platform's own
`<FormType>` in the native tree (22 646 forms; declaration files located by
walking each native tree, slot located with the same block search the Rust
reader uses):

| slot 2 | slot 3 | forms | native `<FormType>` |
|---:|---:|---:|---|
| 0 | 1 | 22 348 | `Managed` |
| 1 | 1 | 289 | `Managed` |
| 0 | 0 | 9 | `Ordinary` |

Per key: `ws` 0 forms, `wms` 5, `mdm` 12, `sslbase` 909, `ssl` 1 163, `do`
2 350, `ut` 5 201, `uh` 13 006. No third value of slot 3 exists anywhere, and
the slot is present on every single form. The nine zeros are exactly the nine
forms the native trees declare `Ordinary`, all of them in ERP УХ 3.2.12.6.

This also disposes of the "storage generation of this particular distribution"
reading: Документооборот КОРП 3.0.21.3, exported by the same platform build,
has 2 350 forms and not one ordinary among them. ERP УХ is not a different
generation, it is the only stand corpus that still ships ordinary forms.

## The fix

* `parse_declared_form_type` (`src/mssql_dump/forms.rs`) reads slot 3 of the
  form's own block and names it `Managed` / `Ordinary`. Anything else returns
  `None` -- fail-closed, never an assumed `Managed`.
* `format_form_source_xml` takes the declared type and prints it; the
  declaration path refuses to emit a form declaration whose record declared no
  type it can name.
* `form_body_asset_paths` routes on the declared type: `Ordinary` gets
  `Ext/Form.bin` written from the inflated body verbatim
  (`SourceAssetKind::InflatedBinary`), `Managed` keeps the rendered
  `Ext/Form.xml`, and an undeclared type becomes
  `SourceAssetKind::UndeclaredFormType`, a typed refusal
  (`form source asset <path> declares no form-type discriminator`).

## Result

`uh`: nine `Ext/Form.bin` files newly written and byte-exact, and the nine
declarations that had been `differing` only because of the hardcoded
`Managed` become exact. `BROKEN = 0` and `extra = 0` on all eight corpora.
