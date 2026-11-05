# A JSX handler that is also a listener

ADR 0645, measured in Chrome: the read warns from a `window` dispatch in a body, and through a bubbling element dispatch. A store Get
is no call, and is held the same way (`Stored`).
