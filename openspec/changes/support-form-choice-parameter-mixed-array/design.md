# Дизайн: mixed FixedArray ChoiceParameters

Schema-layer строго различает:

- `U`: mode `0`, одно поле kind, два non-nil reference UUID;
- `S`: mode `1`, kind и строка, два exact nil UUID.

Presentation и порядок элементов сохраняются в канонической модели.
XML writer выбирает `xr:DesignTimeRef` или `xs:string` по typed variant и не
смотрит на raw discriminator.

Malformed kind, mode, arity, nil/reference side IDs или неполный array
отклоняют весь ChoiceParameters value.
