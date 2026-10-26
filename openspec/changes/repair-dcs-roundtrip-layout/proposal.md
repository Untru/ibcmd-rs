# Preserve DCS values while accepting structural XML layout

## Why

The DCS compiler regenerates indentation for element-bearing containers. Its round-trip gate rejects supported authored schemas that use LF, CR or compact child-element layout even when the stored and exported values are unchanged. It also rejects schemas where the writer replaces an unused prefixed namespace declaration.

## What changes

Compare supported DCS schema documents with exact remaining element markup and exact scalar content after conservatively excluding proven unused prefixed namespace declarations while ignoring whitespace-only indentation in element-only containers. Keep writer input unchanged and retain rejection of real content changes, character references, mixed content and preserved whitespace.
