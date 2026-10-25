# Interface descriptors in SQL and CF source conversion

## Why

Issue #558: Interface rows are unclassified, the Configuration root loses its complete ChildObjects list, and base-free import refuses the descriptors. Related: #544 and the independent format investigation #525.

## What changes

Add a shared, measured Interface descriptor model and schema-owned layout, canonical names and top-level ordering. Read and compile Switchable and the metadata header, preserve the complete root inventory, and refuse malformed or unsupported records explicitly. Validate with synthetic regression tests and native 8.3.27/8.5 controls. tool1cd is research evidence, not a dependency or copied implementation.
