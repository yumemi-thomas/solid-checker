# rc9-reexport-gap-namespace-member

A namespace import whose every use is a member read: `S.untrack` reaches nothing while `S.createLoadingBoundary` names one of the five rc.9 re-exports without declarations (`rc9-reexport-gap-none`); a shadowing `S` is not the import (binder resolution). Expected: the `SC9014` notice carries the gap naming only `createLoadingBoundary`.
