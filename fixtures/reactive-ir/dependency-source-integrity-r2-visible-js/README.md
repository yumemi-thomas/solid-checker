# Visible unselected JavaScript mutation

ADR 0300 review round 1: `allowJs` with `maxNodeModuleJsDepth` 1 brings `dep-wrap` into the program without selecting it; its computed delete through call-bound `this` vetoes the later getter proof (SC9012). Solid signatures are the published rc.13 ones.
