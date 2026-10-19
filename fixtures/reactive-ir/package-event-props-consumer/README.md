# package-event-props-consumer

**Claim (ADR 0207).** A package component whose accepted contract states an `event-handler-props` item runs every `on...` member of its props only at an external event while its guard holds (`as` absent or a string), so a function literal written as such a prop takes `ExecutionRole::EventCallback`; a component `as`, unknown spread, merge or non-`on...` prop is `SC1001` uncertifiable; the import also raises `SC9005` and `SC9014`. Contract authorized out of band; `package.json` byte-identical to `package-merged-props-consumer`'s.
