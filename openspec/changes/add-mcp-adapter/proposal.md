# MCP adapter for ibcmd-rs (#386)

Expose the common configuration engine to AI clients through a genuine local MCP
stdio command, without exposing editor connect/settings overrides to the model.
The existing editor Content-Length JSON-RPC protocol remains separate.

The adapter supports offline Config row fixtures and disposable native file
infobases through the common descriptor reader. File database import requires
operator-owned per-database `agent-write=true`, stages through the guarded common
API and never runs native apply. A published stage invalidates the session.

Issue completion requires reviewed descriptor-relative mutation confinement,
negative permission/race/cancellation tests, and a genuine MCP module-edit →
shared staging → external native apply → native export/source roundtrip. A
functional roundtrip alone does not waive an open security finding.
