# Clean-room exemption and hard boundary

---
status: accepted
---

SolidCraft's rules forbid driving, scripting or taking data from commercial CAD
products "for this project". craft-bridge is **fully exempt inside this
repository**: development may drive SolidWorks, use its public API documentation,
and commit SolidWorks-produced test outputs (synthetic parts only). The exemption
stops at this repo's edge — a **hard boundary**: nothing derived from SolidWorks
(files, measurements, observations, API findings) enters SolidCraft's code,
tests, docs, issues, PRs, comments or attachments, and SolidCraft-side work is
written purely from SolidCraft's own sources and generic, independently authored
requirements.

Considered: keeping strict clean-room (chokes the project — the bridge cannot be
tested without the product); repealing the rule project-wide (undefendable
clean-room claim for SolidCraft, far beyond what this bridge needs).

Consequences: the boundary is recorded in both repos' agent instructions and is
enforced by review discipline; every SolidCraft-side publication is shown to the
owner before filing; the seat runs only committed, reviewed synthetic-corpus
items — never macros or fixtures from unreviewed public PRs.
