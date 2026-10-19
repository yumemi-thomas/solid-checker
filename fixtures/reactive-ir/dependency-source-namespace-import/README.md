# dependency-source-namespace-import

**Claim (ADR 0322).** A namespace import of selected package source (`import * as Widget from "dep-ns/widget"`) whose every runtime reference is a static member (call, read, JSX tag) binds each member as the named import would and loses its `NamespaceValue` obligation (`SC9005`); type-position names are erased. `Widget.readNow(() => count())` is SC1001, `readLater` clean; an escaping namespace or computed member stays uncertifiable. `dep-ns` is a two-file package (peer `solid-js` 2.0.0-rc.13).
