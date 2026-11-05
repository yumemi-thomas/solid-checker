# `execCommand` delivers `input` on its caller's stack

The ruling on ADR 0642, measured in Chrome: on a focused editable it runs `onInput`, an event of the measured list, in the later body's
window and under its owner. Uncertifiable.
