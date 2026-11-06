# dependency-value-unfollowed

**Claim (ADR 0822).** A Get on a value a selected package makes in a way the walk over its source does not close (an accessor installed at run time, a class getter, a proxy, a call nothing resolves) is an uncertifiable SC9012, `dependency-value-unfollowed`. A value descriptor, a class with no getter under the name and a standard container are clean. Importing a package that installs a function on a global that may read a source is `dependency-global-read`, a side-effect import included.
