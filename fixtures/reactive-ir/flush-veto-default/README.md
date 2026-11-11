# flush-veto-default

**Claim (ADR 0613).** A `flush()` in a parameter default is in no function's body and is not module evaluation: it runs at its function's call, under that caller's owner. The census gives it no owner-free class, so it vetoes, and the apply's write (which rc.17 rejects there) is uncertifiable, not certified.

ADR 0961 (PD): the flush in the default (line 5) is itself an SC2008 uncertifiable row (`parameter-initializer-unplaced`): a default runs when a call omits the argument, `drain()` here is called in a component body, and no net reads which calls omit it, so it may run on an effect's apply.
