# argument-sensitive-reads

**Claim (ADR 0280).** A read a helper performs on some completions is decided for one call when that call's arguments decide the branch (proven where it always runs, dropped where it cannot, uncertifiable otherwise; likewise callbacks called on some paths), over an exact argument mapping for parameters the helper never writes: function/literal-true/omitted/ready arguments violate, number/false/given arguments are none, either/unknown/written arguments and an unready `never` guard are uncertifiable.
