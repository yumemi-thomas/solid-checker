# With no owned flush, an apply's delivery has no owner

The negative of `rc21-apply-owned-flush`: the same click in an apply and no `flush()`, so the queue drains on an empty stack, and the
handler's write is legal.
