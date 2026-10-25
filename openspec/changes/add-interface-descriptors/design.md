# Design

Use a separate metadata_model module for the bidirectional Interface descriptor and an ibcmd-schema layout for physical coordinates. Keep SQL/CF discovery as adapters to the same model. The root schema owns XML child ordering; unresolved child collections must not become an apparently valid root with an omitted inventory.

Evidence: tool1cd commit 7c4b91cb7a7bb48a32be00b9f32afe740dada6e1, MetaTree.txt lines 20339, 53946 and 55949; checked against current native descriptors. In the observed modern row, Switchable is inverted (0=true, 1=false). Confirm every supported shape with native output; do not infer complete interface menu support from a descriptor-only sample. No vendor code, private CF, application XML or raw rows are committed.
