# program-boundary-standalone

**Claim (ADR 0193, 0444).** A private package in no workspace is closed: `bump` is placed through the handler that calls it, and the program certifies. The same files under a workspace a sibling depends on are open (`program-boundary-workspace`).
