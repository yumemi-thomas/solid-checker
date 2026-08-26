---
status: accepted
---

# Follow the compiler in Solid's next branch

Solid 2 compiler integration will follow `solidjs/solid/packages/compiler` from an exact `upstream/next` commit through the `yumemi-thomas/solid` fork, replacing the Solid 2 dependency on the former DOM Expressions compiler fork. The fork contains semantic-fact models, output-neutral recording hooks, validation, serialization, and fact tests only: it must not change lowering, generated output, diagnostics, runtime behavior, compiler features, performance, or unrelated implementation. A compiler defect discovered while adding facts is fixed upstream independently and the affected fact remains open until the fork rebases. The existing semantic trace is ported and proven output-neutral before fact improvements land; the Solid 1.x compiler remains a separate pinned fork, and every consumed revision is immutable.
