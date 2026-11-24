# Design

An external package carries a platform version record as well as a kind-specific descriptor. A source-version override on an 8.3 package would retain the old records. The two 8.5 seeds come from the single-string attribute variants of the public coverage corpus. They carry the native version record, descriptor and empty copyinfo for their kind. The compiled load rekeys the seed and replaces its children with those from the input tree.

The decoder determines which descriptors it can read. In particular, the report seed uses v5. A decoder or verification failure stops publication. The measured target for XML 2.21 is 8.5.1.1150. An explicit conflicting target profile or a different exact 8.5 build is rejected.
