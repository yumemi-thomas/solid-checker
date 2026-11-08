// A classic script (no import or export): a non-const `enum` declares a
// global `var`, so `Promise` is this enum's object for every module.
enum Promise {
  Pending,
}
