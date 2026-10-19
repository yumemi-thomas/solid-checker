# Context member through a namespace-member provider

Negative for `context-member-identity` in a closed program: `FixedTheme` renders a second `ThemeContext` provider through the namespace member `themes.ThemeContext`, which is not one exact identifier and cannot be attributed to one context binding, so no provider census is closed and `ThemeLabel`'s read is not attributed to the signal (an unkeyed provider withholds every context project-wide).
