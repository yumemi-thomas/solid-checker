// A global some script or package provides: the declaration names no provider, so a call of it is uncertifiable (`unknown-global`, ADR 0974).
declare function legacyGlobal(cb: () => void): number;
