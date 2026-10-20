# Inferred host: alias-unresolved

Copy of `inferred-host-spa` whose alias target (`~/missing`) is unresolved: the checker reports `browser host not inferred: unresolved static module request` and admits no host, so the snapshot is `certified` with no finding (an admitted host would make it `uncertifiable`). Missing resolver facts and divergent aliases are unknown, not another runtime target. The stub is the audited `2.0.0-rc.13` triple so the status is not masked by an `SC9014` release notice.
