# component-identity-renders / prototype

An accessor installed on `Object.prototype` (`Object.defineProperty`) runs when `icons[name]` falls through to it, with `icons` as `this`, and could call a stored component plainly. ADR 0292's reflective prototype-write veto withholds the object holder, so `Archive`'s body read is uncertifiable.
