# A created `label` delivers its control's JSX handler

ADR 0645, measured in Chrome: the handler runs under the apply's owner and its `flush()` warns. ADR 0640's exclusion is `<a>` alone.
