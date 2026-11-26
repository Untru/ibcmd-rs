# Issue self-service design

## Decisions

Only open, unlocked, unassigned Backlog or Ready issues already in Untru/ibcmd-rs Project 9 admit a new claimant. The command author is the only assignable person. Existing ownership cannot be displaced. A sole assigned owner can release or request review of their own open non-Draft master-targeting PR with an explicit reference to the issue. Backlog claims do not require a prior Ready transition. A sole owner can reconcile a partial Backlog claim with a fresh command; no command closes an issue or grants permissions.

`issue_comment: created` runs trusted default-branch code. Pull-request checks run the same offline tests without secrets or write permissions. The comment is JSON data from the event file and is re-read from the repository API; shell/code interpolation, fork checkout, arbitrary URLs and user-selected tokens/project IDs are excluded. Only repository API operations use GITHUB_TOKEN; only fixed Project GraphQL requests use PROJECT_TOKEN. Missing or denied credentials refuse before Assignees/Status/Agent writes.

GitHub Actions job concurrency uses a fixed repository/issue group, `queue: max`, cancel-in-progress false. Its documented 100-pending-run ceiling is explicit; contributors wait for the bot confirmation and can repost a command whose run never starts. The first successfully accepted claim wins, not a promise about comment arrival order.

Before a state change the bot creates and reads back a bot-owned pending receipt bound to repository, issue, author, comment ID and command digest. Existing receipts and the Agent request watermark prevent duplicate or older commands from replaying after newer commands. Failures retain the receipt; retry uses a new comment. Replies are updated in the same bot-owned receipt, never in a contributor's text.

The issue assignee is ownership authority. Agent contains one bot-owned JSON line; every other line is retained. Project schema/item identity and all requested options are checked. Each API mutation is guarded by a fresh exact state read and is followed by readback. GitHub provides no transaction/CAS across issue assignment and Project fields: observed drift or failed/uncertain writes receive no success acknowledgement and stop further state mutations. A fresh command can reconcile its own partial assignment; manual interference may require maintainer reconciliation. An add-assignee race removes only the actor this command added when the competing owner is detected. No blanket replacement of assignees or Agent text occurs.

## Alternatives

Public Project visibility does not grant write access. Organization teams still require membership. Repository-wide collaborator privileges exceed the needs of claiming an issue. A label-only tracker would not update the working Project. Repository GITHUB_TOKEN cannot access a personal Project; a dedicated project-scoped credential is provisioned once as an Actions secret, never copied from a maintainer CLI login.

## Deployment and limits

Default target: public personal Project Untru/9. A classic PAT with project scope is stored as PROJECT_TOKEN; repository writes use the ephemeral issues:write token. No token values are committed or printed. A protected owner-only workflow_dispatch preflight checks configuration without changing issues. Live acceptance uses a dedicated disposable issue after merge and secret provisioning. Code/CI completion does not claim live activation while the secret is missing.

## Direct editor exclusion

Project field updates have no CAS. A direct editor inside the pre-read/write interval can be overwritten with a success acknowledgement; the guard does not detect that interval. Supported operation therefore requires an owner-confirmed single writer protocol, enforced as an activation gate ISSUE_BOT_SINGLE_WRITER_CONFIRMED=true. All direct editors and other Status/Agent automations must stop while command ingestion is active. For owner maintenance, disable ingestion, drain/cancel and verify every running and queued job terminal, perform/read back maintenance, then enable. The flag is an explicit operator acknowledgement, not a technical lock over the owner's UI. It remains unset until the owner accepts this scope; code/CI completion does not constitute acceptance.
