# Native export queries (#362)

Implement native `infobase config export info/status/objects` without starting the platform. Reuse the shared Config table view, inventory writer and row converters for filedb and SQL. Preserve the custom editor byte-diff API and custom base+sync API.

Acceptance: exact native stdout/file protocols and selection/error semantics on own fixtures, incremental differential tests, private large-corpus aggregate timings, independent review. Private ERP data stays outside Git.
