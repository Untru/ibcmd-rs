# Design

The existing spreadsheet decoder already represents a document with no format pool as count zero. Its writer plan must accept Identity(0) with empty order and map; explicit empty maps and any inconsistent nonempty projection still fail.

The offline round-trip audit uses MetadataSourceContext.moxel_object_refs, which resolves both pictures and style items. Font numbering is unchanged; missing references and out-of-range indices remain errors.

Native paired source/stored evidence identifies Windows ScrollBar as kind 1/code 0, key 0 as either numeric paper or string pageSize, and key 18/value 0 as printAccuracy Auto. Native evidence is retained locally outside Git. Tracked tests use clean-room strings only. Microsoft GetSysColor also defines COLOR_SCROLLBAR as zero: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getsyscolor .

Add page_size and print_accuracy to the existing canonical print settings. Decode the tagged key-0 value by its native Number/Text type; emit paper or pageSize accordingly. The writer rejects a source spelling both alternatives. Key 18 admits only evidenced Auto/0; unknown values fail. Preserve empty pageSize rather than dropping it. XML emission follows native property order, with accuracy after geometry. No generic unknown-property pass-through, dependency, platform profile, schema bypass, or database creation is introduced.
