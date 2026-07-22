# Language native row — 8.3.27.1989

Status: experimental structural evidence. This document does not claim that
the generated row has been accepted by a running 1C platform.

The evidence was collected read-only from the local `_Config` corpus. No 1C
process was started and no database row was written. The only observed
`Language` primary row had these bounded characteristics:

- logical key: the metadata object UUID, without a suffix;
- storage encoding: raw DEFLATE over UTF-8 text, without a UTF-8 BOM;
- compressed bytes: 105;
- plaintext bytes: 144;
- compressed SHA-256:
  `ce471a588f9a7c35bd0b6b2c06de8ab3062c20b120193cc9ad117b3cb192db3e`;
- complete redacted grammar:

```text
{1,{0,{3,{1,0,<object-uuid>},"<name>",<synonyms>,"<comment>",
0,0,00000000-0000-0000-0000-000000000000,0},"<language-code>"},0}
```

The committed compiler layout `language-v1-crlf-no-bom` emits the same value
tree with deterministic CRLF formatting. Its strict decoder requires all
discriminators, field counts, the nil UUID, canonical lowercase object UUID,
and a synonym count that exactly matches its pairs. Unknown fields are an
error rather than being ignored.

The checked fixture covers XML dialects 2.20 and 2.21 and the complete path
XML -> canonical Language -> native blob -> native IR -> XML -> canonical
Language. Platform acceptance remains a separate promotion gate for this
experimental profile.
