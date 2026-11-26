# EDT project conversion (milestone 0.7)

Implement issues #355–#358: an offline EDT project reader, EDT to Designer XML,
Designer XML to an importable EDT project, and an independently captured EDT
oracle on BSP and ERP UH. Reuse the project-owned Rust XML/EDT codecs from
Segate-ekb/morph1c, pinned at 962eedddd14493a914bae11f11667951de0f23a7.

Production stays standalone Rust. EDT and the 1C platform are lab oracles only.
Existing MSSQL/CF implementations, canonical contracts and profile coordinates
remain authoritative. Borrowed format-local representations are private adapter
details, not a second application-wide canonical model.

No release or completion claim is based on synthetic round trips alone. Every
GitHub issue requires its own reproducible acceptance evidence.
