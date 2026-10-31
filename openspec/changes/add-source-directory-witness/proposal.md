# Add a detached source-directory witness seam

## Why
The shared source census needs directory membership evidence in addition to its file inventory (#382, #386). Its parent module cannot traverse directories using the private retained RootAnchor descriptor. Opening a fresh absolute RootAnchor for every member would change the anchoring contract. A small read-only primitive is needed before the independently owned census can retain bounded directory evidence.

## What changes
Add only a crate-private RootAnchor::directory_witness method, its opaque detached equality type and owned API tests in a separate donor based on reviewed #533 head 0cb96333e653c79d7f13d7d0200a79833538bcb4. Specify the contract before implementation. Keep existing open_file, selected/detached file witnesses, directory/leaf access and sharing, source profile admission and publication routes unchanged. No public API, dependency, CLI, writer, MCP selector, workflow or donor-history change is part of this seam.
