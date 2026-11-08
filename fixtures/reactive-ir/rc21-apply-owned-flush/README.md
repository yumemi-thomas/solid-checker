# An apply's delivery under an owned flush has that owner

ADR 0639 with ADR 0613, measured in Chrome (`g10`): a `flush()` in a later body drains the apply, its `click()` delivers the JSX handler
under that body's owner, and the handler's write throws. The handler is uncertifiable.
