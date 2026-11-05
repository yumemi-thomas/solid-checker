# SC5001: a source that may answer synchronously

`can_return_async` holds for a union with a thenable. A compute that may return a plain
value or a custom thenable settles on its call in Chrome, so no first read is proven
pending: uncertifiable. A native `Promise` and an `async` function stay proven (ADR 0631).
