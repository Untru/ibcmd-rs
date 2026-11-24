# Windows BSP oracle on EDT 2026.1.3.25

On 2026-11-24, installed EDT imported BSP 3.1.9.232 (XML 2.20, runtime
8.3.27) with build enabled, exported 8,566 XML-tree files and completed
validation. Both converter entry points on master `54b796f6` rejected an
ExchangePlan during preflight. Neither published an output tree. These
results locate the first blocker for each direction of #358.

| Route | Result on this corpus |
| --- | --- |
| Authentic EDT → converter XML vs fresh native export of the EDT baseline | Exit 2. 95 uncovered `standardAttributes` source cells in one ExchangePlan. No candidate XML to compare. |
| Native XML → converter EDT → installed EDT export vs installed EDT baseline | First conversion exited 2. 295 uncovered `ExchangePlan/Properties/StandardAttributes` source cells in one ExchangePlan. No generated project to strip, export or validate. |
| Native XML → converter EDT → unchanged converter XML | Same failed generation step. Byte equality, including CDI, remains unmeasured. |

The installed EDT baseline differs from the original native input on **335
paths**: 333 files differ in content and two exist only in the native input.
One of those is `ConfigDumpInfo.xml` (2,151,471 bytes). The full input manifest
matched again after the run. These counts describe the two available trees.
A three-way report requires a candidate output.

The successful import, export and validate commands all returned zero.
Authentic validation produced a zero-byte TSV. The corresponding workspace
log retained three severity-4 headless lifecycle records and four severity-8
state-update records. There was no generated-project diagnostic differential
or empty ambient control. The initial failed import has a separate workspace
and log.

The native control used a new file database created by Designer 8.3.27.2214.
Its load of the installed EDT export exited 1 with a missing-license message.
The fresh post-EDT native reference is therefore unavailable on that build. A second, separate file database attempt with native ibcmd 8.3.27.2342 timed out during creation after 900 seconds.

The CLI reported `2026.1.3.25`. The candidate used the explicit
`edt-2025.2.3-xml-2.20` profile for this compatibility probe. The original
2025.2.3 acceptance, XML 2.21 and ERP УХ remain open. The machine's platform
binary versions, input and output inventories, raw report digests and
diagnostic counts are recorded in the [evidence summary](evidence/358-windows-bsp-20261124.json).
Commercial trees and raw lab logs are retained privately.

## Reproduction

Use an empty run directory, the native BSP XML tree and a candidate built from
the revision in the accompanying evidence. Keep raw inventories, commands,
validation TSVs and commercial configuration files in the private lab.

1. Record the installed CLI's `version`. Run its `import --version 8.3.27
   --configuration-files <native> --project-name OracleConfiguration --build true`
   in a fresh workspace. Run `export --project-name OracleConfiguration
   --configuration-files <baseline>` in that workspace.
2. Create an empty file infobase. Load the baseline using Designer
   `/LoadConfigFromFiles` and export using `/DumpConfigToFiles <fresh-xml> -Format Hierarchical`.
   Compare this fresh reference with `convert` of the authentic EDT project
   to XML. Retain the raw CDI row and record body differences separately.
3. Convert the original native XML to EDT. Return the unchanged generated
   project to XML and compare every path and byte with the input, including
   `ConfigDumpInfo.xml`.
4. Copy the generated project without its root `.ibcmd-provenance`.
   Use installed EDT to export that copy in a separate workspace and compare
   it with the installed EDT baseline. Validate both projects with
   `validate --file <file.tsv> --project-list <project>`, retaining each TSV
   and the workspace logs.

Both converter directions use explicit `xml-2.20` and
`edt-2025.2.3-xml-2.20` profiles. Tree hashes and diagnostic multiset
comparisons use `scripts/edt-lab/oracle.py`. A successful process exit alone
does not establish equality or clean validation.

### Windows launcher setting

The first isolated import on this host recorded
`java.io.IOException: Unable to establish loopback connection` and repeated
60 s XML formatter service timeouts. With the supplied Axiom JDK 17.0.16,
`java.nio.channels.Pipe.open()` reproduced the socket exception by itself.
Create a short, ASCII-only socket directory under the disposable run and set
`JAVA_TOOL_OPTIONS=-Djdk.net.unixdomain.tmpdir=<absolute-socket-directory>`
in the EDT process environment. The pipe check then succeeds. The first
partial workspace was retained privately and a fresh import started from the
same immutable native input.
