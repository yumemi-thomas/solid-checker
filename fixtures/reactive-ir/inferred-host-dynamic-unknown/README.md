# Inferred host: dynamic-unknown

Copy of `inferred-host-spa` whose Vite config contains an unknown plugin spread in a dynamic branch: browser admission is refused, so the snapshot is `certified` with no finding. The admitted twin is `inferred-host-dynamic` (`uncertifiable`); the status difference between the two is the pin, since `startClosed(): void` carries no ownership requirement (ADR 0312).
