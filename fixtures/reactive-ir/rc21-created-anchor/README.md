# A created anchor delivers to no listener

ADR 0640. `download` creates an anchor, attaches it under `document.body`, clicks it and
removes it. Nothing in the program listens on that path, so the dispatch withholds no proof:
the handler's read and write stay certified.
