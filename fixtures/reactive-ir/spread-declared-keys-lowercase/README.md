# spread-declared-keys-lowercase

Claim (ADR 0436): which declared keys are listeners is the dialect's answer (`spread_key_is_event`). On the rc.17 web runtime only `on` followed by an uppercase letter installs a listener, so a spread whose declared type has only `class` and `onclick` is handler-free and nothing reports. The twin `spread-declared-keys-lowercase-rc13` is the same source on rc.13, where `onclick` is a listener.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
