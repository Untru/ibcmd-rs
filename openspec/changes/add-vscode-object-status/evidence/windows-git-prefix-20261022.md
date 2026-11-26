# Windows Git prefix follow-up

The historical exact-head Windows client run at ba747e63 failed the real Git
baseline unit test with `Source is outside Git repository`; its Git helper blob
was unchanged at the later 41a78172 publication head. Linux passed. Git's native
path spelling can differ from Node's Windows temp/source path (8.3 aliases or
junctions), so subtracting its --show-toplevel from the source is unreliable.

The adapter now asks Git for --show-prefix directly. It validates the relative
prefix before materialization and preserves spaces, without changing Git/global
configuration, native status semantics or shell-free argv execution. A real
Git regression traverses a directory alias/junction and a source directory with
spaces, then proves committed bytes win over locally modified bytes. Existing
missing-ref, cancellation, subtree filtering and unsupported-entry guards remain.

Historical failure evidence: Jimmo910/ibcmd-rs workflow run 39823169104,
Windows job 119979451432. It is diagnostic evidence, not a claim about subsequent
head CI. Current-head CI must be rerun; the PR remains draft.
