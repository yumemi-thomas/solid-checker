# read-after-await-premise

ADR 0349 premises: see App.tsx.

ADR 0951: `variants/tripwire-define` still withdraws the Promise premise, by its own tripwire (the SC1002 `handed-read-after-await` row at the handler); the veto no longer withholds the calls of members the definer (`all`) does not name, so the rows it masked it with are gone.
