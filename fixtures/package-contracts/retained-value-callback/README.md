# Collection storage does not prove queued invocation

Map, Set and Array retain each input without calling it. Their callbacks must
remain unknown, with no invoke proposal. Direct invokes its input and keeps its
callback proposal. The native test verifies the normal document and refuses a
direct callback claim transplanted onto any storage export. The declarations
are complete signatures, with no looser callback return type than the code.
