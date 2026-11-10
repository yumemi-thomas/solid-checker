# Bare global listener and dispatch

ADR 0937: lib.dom's global `addEventListener` and `dispatchEvent` are the window's: a bare dispatch runs the listener
synchronously (Chrome: STRICT_READ_UNTRACKED in each). U (`listener-reach`); was silent.
