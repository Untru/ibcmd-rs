# Witnessed current DCS structural framing

The post-DCSSCA authentic reverse conversion publishes its output, but its DCS
Template.xml differs from the bound native 8.5.1.1592 export by exactly 75 LF versus
CRLF separators (3,189 versus 3,264 bytes). Strict comparison remains failed.
The source was captured with installed EDT 2026.2.1.3rc0.

The prior pipeline assumed structural CRLF in both dialects. A declaration-bound
current EDT source facet now projects eligible element-only structural whitespace
to canonical CRLF on attachment and restores LF for same-EDT output. Designer
emission uses the complete canonical body. No semantic comparator is loosened.
Leaf text, CDATA, mixed content, attributes and inherited xml:space-preserved
scopes retain their bytes; explicit xml:space=default resets inheritance. A source
with mixed eligible structural framing is rejected because one flag cannot retain
its per-boundary spelling. Existing text-node and QName projection policies remain.
Native-origin LF sources and legacy CRLF sources retain their existing behavior.

All eleven focused DCS/form/DCSS/DCSSCA controls pass under Rust 1.95, jobs=2.
The authentic 3,186-byte EDT source emits the exact 3,264-byte bound native
template (SHA-256
`05c5fe4b67aaf6eb46bbf644fe8872f281a046ff1c236714c7ea0e675d58c635`)
and the original EDT bytes on same-format output. Both inputs remain unchanged. The separate two module framing
differences belong to #357. ConfigDumpInfo remains a separate strict discrepancy.
This change does not establish BSP/UH coverage or full current-schema/runtime
acceptance, and no legacy profile is renamed.
