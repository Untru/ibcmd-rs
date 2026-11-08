# Export empty DCS picture values

## Why

#618 identifies a valid empty DCS picture that the XML 2.21 writer refuses.
Fresh native verification uses 8.5.1.1529; its empty readable value differs
from the pinned 8.5.1.1150 oracle. That measurement boundary must remain explicit.

## What changes

Accept the measured empty stored primitive and write the current native empty
readable value. Accept that readable value on import. Preserve named-picture
rules, strict refusal boundaries and every pinned legacy oracle. Record the
remaining legacy difference instead of claiming complete 1150 equality.
