---
title: Agent workspace
status: current
version: 0.1.0
last_updated: 2026-11-23
last_verified: 2026-11-23
source_refs: []
owner: "@rfluid"
tags: [agents]
---

# `.agent/`

Everything an AI agent needs to work on Detent, beyond the human docs in `docs/`.

| Dir | What | When to read |
|---|---|---|
| [`context/`](context/) | Stack map, conventions, glossary | Always, before the first change |
| [`skills/`](skills/) | Step-by-step procedures for recurring changes | When the task matches a skill |
| [`workflows/`](workflows/) | Multi-step processes (implement a phase, change the protocol) | For larger tasks |
| [`prompts/`](prompts/) | Reusable prompts (e.g. one-shot bootstrap) | When delegating to another agent |
| [`memory/`](memory/) | Facts, lessons and patterns learned while working | Always skim `INDEX.md`; append after non-trivial tasks |

Skills: [add-hidpp-feature](skills/add-hidpp-feature.md) · [add-native-plugin](skills/add-native-plugin.md) · [hot-path-change](skills/hot-path-change.md) · [verify-on-device](skills/verify-on-device.md)

Workflows: [research-prior-art](workflows/research-prior-art.md) · [implement-phase](workflows/implement-phase.md) · [change-protocol](workflows/change-protocol.md)
