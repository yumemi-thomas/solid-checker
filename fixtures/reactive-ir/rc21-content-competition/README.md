# SC8003: which content sources lose something (ADR 0651)

Measured in Chrome on rc.21. `innerHTML` or `innerText` beside a real child, a
filled `textContent` beside any child, and two filled attributes are proven. A
blank child beside `innerHTML` loses nothing and is silent. A value not proven
filled, and a spread that may carry a content key, are uncertifiable.
