# Form Diff Matrix

The faster Form.xml path is a matrix of small controlled differences, not one
large form reviewed by hand.

## Workflow

1. Keep a baseline Form.xml and its raw deflated Form body blob.
2. Create one variant Form.xml with one property or shape changed.
3. Produce the matching native Form body blob from the platform.
4. Run `form-diff-candidates` against the two XML/blob pairs.
5. Inspect the JSON candidate `XML path -> layout path` mapping.
6. Add a focused pack/unpack test for that mapping.
7. Use a large real form only as regression evidence after the small mapping is
   covered.

## Command

```powershell
target\debug\ibcmd-rs.exe form-diff-candidates `
  --base-xml E:\ibcmd_lab\forms\base.xml `
  --variant-xml E:\ibcmd_lab\forms\readonly_true.xml `
  --base-blob E:\ibcmd_lab\forms\base.bin `
  --variant-blob E:\ibcmd_lab\forms\readonly_true.bin `
  -o E:\ibcmd_lab\forms\readonly_true_candidates.json
```

Inputs are raw deflated Form body blobs from `Config` or `ConfigSave`, not
inflated text files. The command parses Form.xml leaf values, parses the Form
layout brace tree, diffs both sides, and emits candidate mappings.

## Current Limits

- Candidate confidence is high only when exactly one XML leaf and one layout
  atom changed.
- Medium candidates are value-based and still need human review.
- It does not run ibcmd or generate native variants by itself.
- It is a mapping accelerator for the current staging-over-existing-base path,
  not a blank-infobase bootstrap importer.
