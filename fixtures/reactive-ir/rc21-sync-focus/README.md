# focus() delivers a handler on its caller's stack

ADR 0642, measured in Chrome. `input.focus()` from a component body runs the input's `onFocus` there,
under the body's owner and in its strict-read window: Solid warns on the read and throws on the write.
The handler's read and write are now uncertifiable.
