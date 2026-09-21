# The parameters and commands sections, measured over ERP УХ

Date: 2026-09-21. Tools: `F:\ibcmd\lab\tools\try-parameters.py`,
`try-commands.py`. Reports: `param-try.txt`, `cmd-try.txt`.

All four list sections of the frame -- parameters, commands and the two
appearance sections -- have the same shape in all 12 507 bodies:

```text
{0,<count>,<record> x count}
```

with **nothing** after the records. The parameters section holds
`{0,"<name>",…}` records, the commands section `{9,{<id>,409b9a53-…},…}`
records in the command namespace, and both appearance sections `{3,0,{0,<uuid>},…}`
records.

## Parameters

```
parameters 24863, exact 24863 (100.00%), forms whose counts differ 0
record widths: [(4, 24863)]
```

Four members, always: `{0,"<name>",<type pattern>,<key>}`, where `<key>` is 1
for `<KeyParameter>true</KeyParameter>`. The type pattern names configuration
types, so the caller supplies it; everything else is read from the source, and
**all 24 863 records rebuild byte for byte**.

## Commands

```
commands 61228, exact 60983 (99.60%), forms whose counts differ 0
record widths: [(19, 61228), (21, 2)]
```

Nineteen members:

```text
{9,{<id>,<command namespace>},"<name>",<title>,<tooltip>,<use always>,
   <picture index>,<picture>,"<action>",<representation>,<modifies saved data>,
   0,<functional options>,<current row use>,<member 14>,1,0,0,<current row use>}
```

`<Representation>` is `Text` 0, `Picture` 1, `TextPicture` 2, and 3 when the
command says nothing.

**`<CurrentRowUse>` is written in two places, and the two do not agree.**
Member 13 is 0 only for `Use`, and 1 otherwise. Member 18 is 0 for `Use`, 1 for
`DontUse` and 2 when the command says nothing. Reading either place alone -- or
reading both the same way -- leaves 61 057 of the 61 228 records wrong. This is
the third property of the body found to be written twice under two different
codings, after `<VerticalScroll>` and `<Group>` in the root tail.

The 245 records still unaccounted for differ only in member 14, which takes
values from 1 to 325 and which the command's own element does not carry. The
writer takes it from the caller, defaulting to the 0 that the other 60 983
write.
