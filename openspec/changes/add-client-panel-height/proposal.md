# Preserve the evidenced client application panel height

## Why

The client application interface writer rejects a panel with `height`, and the reader drops the corresponding stored coordinate. An otherwise unchanged interface can therefore differ from its target export and block a configuration import.

## What changes

Represent the witnessed optional panel height in both directions. Keep an absent height as the existing native tail `2,0`; encode and export explicit height `1` as `2,1`. Reject unmeasured heights and malformed records rather than guessing their meaning or dropping their values.
