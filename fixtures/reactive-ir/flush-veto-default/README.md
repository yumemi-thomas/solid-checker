# flush-veto-default

**Claim (ADR 0613).** A `flush()` in a parameter default is in no function's body and is not module evaluation: it runs at its function's call, under that caller's owner. The census gives it no owner-free class, so it vetoes, and the apply's write (which rc.17 rejects there) is uncertifiable, not certified.
