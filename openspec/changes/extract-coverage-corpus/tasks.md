# Extract Coverage — implementation plan (#566)

Design: design.md. Execute sequentially.

- [x] 1. Snapshot the tracked corpus, tools and all baseline hashes at ab2834c4.
- [x] 2. Add and observe a failing checkout contract before the submodule exists.
- [x] 3. Populate Untru/1c-fixtures with public artifacts, native tools, provenance and standalone validation.
- [x] 4. Keep every converter baseline byte-identical and split source-probe expectations by stable ID.
- [x] 5. Publish the corpus commit, add the pinned HTTPS submodule and adapt test/tool paths.
- [x] 6. Preserve existing workflow jobs/matrices; initialize the submodule and add fail-closed validation.
- [x] 7. Verify every source file's disposition/hash, negative checkout/baseline cases and a clean recursive clone.
- [x] 8. Run cargo xtask check/ci, independent review, and publish the migration PR and project status.
- [x] 9. Rebuild the current converter branch, measure all three combined Coverage artifacts and refresh the readable audit with current refusals and corpus class matches.
