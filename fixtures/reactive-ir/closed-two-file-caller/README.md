# A caller in another file with no owner edge (ADR 0868, reviewer's R38)

In a closed program an export has no open-world seed, so the owned edge from `App` looks
like every entry. `hold(() => wrapUse())` is a second entry, from a function no edge
reaches: `wrapUse` is uncertifiable. `placedUse`, called only from `App`, stays silent.
The fuzzer cannot express this: its programs are open-world, and it reads App.tsx rows only.
