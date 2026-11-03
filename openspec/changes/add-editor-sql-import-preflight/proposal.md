# Guard SQL editor partial-import preparation (#382)

## Why

The editor import preparation route currently accepts only file databases. The
legacy SQL object route replaces ConfigSave and does not bind a confirmed plan
to an immutable target readset. Calling it after a separate read-only check
would lose unselected staging and leave a check/write race across SQL sessions.

## What changes

- Add bounded SQL main-configuration preparation for exactly one existing
  `CommonModules/<name>/Ext/Module.bsl` or one Document-owned
  `Documents/<document>/Forms/<form>/Ext/Form/Module.bsl` in the admitted cohort.
  Object selections, synchronized changed selection and multiple selections are
  unsupported in this slice.
- Pin source custody, connection generation, actual SQL database identity and the
  complete CONFIG/CONFIGSAVE compiler readset in an opaque one-use plan.
- Preserve existing staging outside selected row keys; validate references and
  require separate dependency expansion consent followed by a new preparation.
- Publish only through a single-session transaction that verifies identity and
  exact readset/staging CAS under locks before any target DML, then applies the
  verified selected rows without dropping unselected staging.
- Keep dynamic update/apply (#347/#384), Windows filedb mutation and renderer
  form-body changes outside this slice. SQL capability remains unavailable until
  the guarded adapter and disposable SQL acceptance have evidence.
