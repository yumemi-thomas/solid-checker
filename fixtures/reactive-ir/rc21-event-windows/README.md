# The strict window and the current DOM event

Measured in Chrome on rc.21 (ADR 0639). A body's window takes any event; an effect's
records its queue's event, so a handler dispatched from an apply, a timer and a microtask
read in none, and the apply's own read warns, after a dispatch too.
