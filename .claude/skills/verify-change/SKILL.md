---
name: verify-change
description: Prove an ibcmd-rs change works before calling it done. Picks the tests the diff needs, runs the check loop, separates your failures from failures that already exist on the merge base, and writes the evidence. Use after implementing or fixing anything in ibcmd-rs, when a check or test fails, or when asked whether a change works.
---

Read `.agents/skills/verify-change/SKILL.md` and follow it. That is the one copy every agent tool shares; change it there, and keep the name and description above equal to its own.
