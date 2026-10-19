# A consumer of member-path `callbacks` items

An accepted contract whose closed `callbacks` includes a call of a member of an argument (`from: {arg: 1, path: ["0"]}`, `@kobalte/utils`' `callHandler`) folds the member's value as an inline call exactly when the call's syntax names it, never as a call of the argument itself: direct/handler/pair/bound-pair calls are `SC1001`; whole and stored arguments clean; open variant `SC9005`. `callHandler` is declared as `@kobalte/utils@2.0.0-alpha.0` does (`JSX.EventHandlerUnion` reduced).
