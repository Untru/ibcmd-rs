# Предложение: mixed FixedArray в ChoiceParameters

## Зачем

Immutable run `20260726_full_54ca4ee_g_ut` доказал обычный Form
`ChoiceParameters` FixedArray со смешанными элементами:

```text
[DesignTimeRef, String]
```

Прежний decoder принимал только `U` design-time reference и корректно оставлял
весь параметр opaque на втором `S` элементе.

## Что меняется

- Канонический элемент массива становится typed sum `DesignTimeRef | String`.
- Decoder принимает только exact observed `S` grammar с nil side IDs.
- Writer сохраняет исходный порядок и эмитит строку как `xs:string`.
- Никаких правил по имени формы, параметра или строковому значению.

## Evidence

- form UUID `13acd88b-9833-487c-b876-b99f21fd4436`;
- native parameter `Отбор.ТипОборудования`;
- native order: enum DesignTimeRef, затем `ПринтерЧеков` как `xs:string`;
- raw array item kinds в том же порядке: `U`, `S`.
