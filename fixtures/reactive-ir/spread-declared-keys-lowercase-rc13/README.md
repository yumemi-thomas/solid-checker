# spread-declared-keys-lowercase-rc13

Claim (ADR 0394): which declared keys are listeners is the dialect's answer. On web rc.13 any `on`-prefixed spread key installs a listener, so the declared `onclick` key is a possible handler and the spread keeps its `missing-owner` obligation (SC4001, uncertifiable). The twin `spread-declared-keys-lowercase` is the same source on rc.17, where `onclick` is an attribute and nothing reports.

The stub packages carry only the version (2.0.0-rc.13) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
