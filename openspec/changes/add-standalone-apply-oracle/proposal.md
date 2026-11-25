## Why
The standalone acceptance matrix needs to exercise the drop-in CLI entry point
against a real ibsrv while the released product has no standalone transport.

## What Changes
Add an explicit platform-oracle route for local config apply. Keep the
default release route and its no-platform execution boundary.

## Impact
Oracle builds gain one opt-in environment setting for native comparisons.
