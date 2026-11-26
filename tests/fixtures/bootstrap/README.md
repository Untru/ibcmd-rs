# Standalone bootstrap corpus

This directory contains the checked-in source corpus used to prove the
base-free `cf bootstrap` boundary. The fixtures are hand-authored clean-room
data and contain no third-party application code.

`manifest.json` pins every fixture to independent platform-profile and XML
dialect coordinates, lists the complete source inventory, and lists the exact
native storage inventory expected after compilation. Tests execute the public
CLI with an empty `PATH`, inspect the generated CF, export it back to a source
tree, and compare the complete relative-file set.

The corpus is deliberately small. Passing it proves the declared profile and
family slice only; any source file without an exact registered route remains a
hard bootstrap blocker.
