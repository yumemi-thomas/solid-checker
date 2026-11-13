import { createEffect, createMemo } from "solid-js";

const pause = () => Promise.resolve();

// This program holds a tracked async computation with something after a suspension (`Tracked`): the callee reason's premise (the member-route net, ADR 0963, runs) holds, so the
// chains below are the only thing that decides whether a call ends.
export function Tracked() {
  const m = createMemo(async () => {
    await pause();
    return 1;
  });
  createEffect(
    () => m(),
    () => {},
  );
  return <div />;
}

// Batch 11, slice 9B (ADR 0973): the callee summary follows a chain of exactly resolved project calls at most 64 functions deep (`MAX_CLOSURE_DEPTH`, a bound on the search's stack); a longer
// chain is not concluded about (`blocked:depth`, never memoized) and its call keeps the uncertifiable row of SC1002. A chain of 60 pure functions is within the bound: the call ends and
// has no row; a chain of 67 is beyond it. (An open program boundary, `main` in package.json: main's proof log, a gate-only mode, grows exponentially with a call chain's depth in a closed
// one, so a chain this long cannot live in a `private` fixture.)
function e0() { return e1(); }
function e1() { return e2(); }
function e2() { return e3(); }
function e3() { return e4(); }
function e4() { return e5(); }
function e5() { return e6(); }
function e6() { return e7(); }
function e7() { return e8(); }
function e8() { return e9(); }
function e9() { return e10(); }
function e10() { return e11(); }
function e11() { return e12(); }
function e12() { return e13(); }
function e13() { return e14(); }
function e14() { return e15(); }
function e15() { return e16(); }
function e16() { return e17(); }
function e17() { return e18(); }
function e18() { return e19(); }
function e19() { return e20(); }
function e20() { return e21(); }
function e21() { return e22(); }
function e22() { return e23(); }
function e23() { return e24(); }
function e24() { return e25(); }
function e25() { return e26(); }
function e26() { return e27(); }
function e27() { return e28(); }
function e28() { return e29(); }
function e29() { return e30(); }
function e30() { return e31(); }
function e31() { return e32(); }
function e32() { return e33(); }
function e33() { return e34(); }
function e34() { return e35(); }
function e35() { return e36(); }
function e36() { return e37(); }
function e37() { return e38(); }
function e38() { return e39(); }
function e39() { return e40(); }
function e40() { return e41(); }
function e41() { return e42(); }
function e42() { return e43(); }
function e43() { return e44(); }
function e44() { return e45(); }
function e45() { return e46(); }
function e46() { return e47(); }
function e47() { return e48(); }
function e48() { return e49(); }
function e49() { return e50(); }
function e50() { return e51(); }
function e51() { return e52(); }
function e52() { return e53(); }
function e53() { return e54(); }
function e54() { return e55(); }
function e55() { return e56(); }
function e56() { return e57(); }
function e57() { return e58(); }
function e58() { return e59(); }
function e59() { return 1; }
export async function Within() {
  await pause();
  e0();
}

function d0() { return d1(); }
function d1() { return d2(); }
function d2() { return d3(); }
function d3() { return d4(); }
function d4() { return d5(); }
function d5() { return d6(); }
function d6() { return d7(); }
function d7() { return d8(); }
function d8() { return d9(); }
function d9() { return d10(); }
function d10() { return d11(); }
function d11() { return d12(); }
function d12() { return d13(); }
function d13() { return d14(); }
function d14() { return d15(); }
function d15() { return d16(); }
function d16() { return d17(); }
function d17() { return d18(); }
function d18() { return d19(); }
function d19() { return d20(); }
function d20() { return d21(); }
function d21() { return d22(); }
function d22() { return d23(); }
function d23() { return d24(); }
function d24() { return d25(); }
function d25() { return d26(); }
function d26() { return d27(); }
function d27() { return d28(); }
function d28() { return d29(); }
function d29() { return d30(); }
function d30() { return d31(); }
function d31() { return d32(); }
function d32() { return d33(); }
function d33() { return d34(); }
function d34() { return d35(); }
function d35() { return d36(); }
function d36() { return d37(); }
function d37() { return d38(); }
function d38() { return d39(); }
function d39() { return d40(); }
function d40() { return d41(); }
function d41() { return d42(); }
function d42() { return d43(); }
function d43() { return d44(); }
function d44() { return d45(); }
function d45() { return d46(); }
function d46() { return d47(); }
function d47() { return d48(); }
function d48() { return d49(); }
function d49() { return d50(); }
function d50() { return d51(); }
function d51() { return d52(); }
function d52() { return d53(); }
function d53() { return d54(); }
function d54() { return d55(); }
function d55() { return d56(); }
function d56() { return d57(); }
function d57() { return d58(); }
function d58() { return d59(); }
function d59() { return d60(); }
function d60() { return d61(); }
function d61() { return d62(); }
function d62() { return d63(); }
function d63() { return d64(); }
function d64() { return d65(); }
function d65() { return d66(); }
function d66() { return 1; }
export async function Beyond() {
  await pause();
  d0();
}
