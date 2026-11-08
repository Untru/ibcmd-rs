# Restore 8.5 form trailers with generated optional panels

## Why

Refs #602. Fourteen public HierarchyPanelLocation=None cases have the three
stored generated root panels. Their revision-59 trailer grows by one record
per present panel, so a fixed distance from the end fails to select the 8.5
reader. The table's known hierarchy member is then never read. The storage
recipe and native fixtures retain those panels although native XML does not
publish them.

## What changes

- Recognize the measured revision-59 root trailer through a schema-owned
  bounded sequence of twelve logical members and three optional panel slots.
- Validate each present generated empty panel and retain its classification;
  normalize physical positions before the existing 8.5-to-8.3 conversion.
- Refuse malformed, ambiguous or unknown declared-8.5 trailer shapes before
  source publication instead of falling through to the generic reader.
- Hold all 35 public positive hierarchy cases, zero-panel controls and
  negative trailer cases. Re-measure the whole corpus and both native builds.
- Do not add a workflow gate or change a vendor snapshot or generated JSON.
