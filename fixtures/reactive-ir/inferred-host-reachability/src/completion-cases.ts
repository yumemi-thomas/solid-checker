// Each outer task is independent: one certain exit cannot kill another twin.
setTimeout(() => {
  void "completion client exit prefix";
  function stop() { if (!import.meta.env.SSR) throw 0; }
  stop();
  void "completion client exit tail";
  setTimeout(() => { void "completion client exit nested timer"; }, 0);
}, 0);
setTimeout(() => {
  function stop() { if (import.meta.env.SSR) throw 0; }
  stop();
  void "completion server exit client tail";
  setTimeout(() => { void "completion server exit feasible timer"; }, 0);
}, 0);
setTimeout(() => {
  function stop() { if (true) throw 0; }
  stop();
  void "completion literal exit tail";
}, 0);
setTimeout(() => {
  function stop() { throw 0; }
  stop();
  void "completion unconditional exit tail";
}, 0);
setTimeout(() => {
  function stop() { while (true) {} }
  stop();
  void "completion infinite loop tail";
}, 0);
setTimeout(() => {
  function stop(value: boolean) { if (value) throw 0; }
  stop(true);
  void "completion literal argument exit tail";
}, 0);
setTimeout(() => {
  function stop(value: boolean) { if (value) throw 0; }
  stop(false);
  void "completion literal argument normal tail";
}, 0);
setTimeout(() => {
  function stop() { if (true) return; throw 0; }
  stop();
  void "completion normal return tail";
}, 0);
setTimeout(() => {
  async function stop() { throw 0; }
  void stop();
  void "completion async rejection tail";
}, 0);
setTimeout(() => {
  function* stop() { throw 0; }
  stop();
  void "completion generator allocation tail";
}, 0);
setTimeout(() => {
  function stop() { stop(); throw 0; }
  stop();
  void "completion recursion feasible if returning";
}, 0);
setTimeout(() => {
  const object = { stop() { throw 0; } };
  object.stop();
  void "completion method feasible if returning";
}, 0);
setTimeout(() => {
  function stop() { if (!import.meta.env.SSR) throw 0; }
  function outer() { stop(); }
  outer();
  void "completion nested exact exit tail";
}, 0);
setTimeout(() => {
  class Stopped { static value = (() => { throw 0; })(); }
  void Stopped;
  void "completion unknown class initialization tail";
}, 0);
