# 8.5 compatibility inventory

## Runtime

- Installed/native tools: 8.5.1.1150.
- Source cluster/RAS: `localhost:3541` / `localhost:3545`.
- Source BSP UUID: `44c05fe0-b4a4-4746-acda-0cd3b4b4da38`.
- The source worker serves four non-zero infobases and is not eligible for
  worker activation.
- 8.5 RAC retains the blank-line block format and the `process`, `connection`,
  and `infobase` keys used by the current parser.

## Product gaps

- Main dump, stage, apply, activation, and watch commands carry XML dialect but
  no platform/storage profile.
- Extension load hardcodes `platform-8.3.27.1989`.
- Main generation, marker, row-cohort, and recovery codecs are explicitly
  evidenced only for 8.3.27.
- Extension registry, CAS, configinfo, timestamp, and activation offsets are
  explicitly evidenced only for 8.3.27.
- `platform-8.5.1.1150` is currently an experimental identity stub with no
  storage profile, constants, fingerprints, or capability evidence.
- The dump decoder rejects packed reference versions above 80327 and managed
  form coverage is incomplete independently of platform identity.

## Minimum evidence gate

Every MSSQL write must select an exact platform profile and validate a stable
SQL schema fingerprint before active export or staging. XML 2.21 does not imply
platform 8.5. Main overlay, managed-form overlay, extension dump/load, extension
activation, and worker handoff are separate capabilities and are enabled only
after their own native evidence passes.

The acceptance matrix records table/column fingerprints, service row codecs,
native/custom source parity, main module and form snapshots, extension CAS and
registry snapshots, same-session marker observation, shared-worker refusal,
recovery round-trip, timing, and complete clone cleanup.
