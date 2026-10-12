# Context member identity, open program

Control for `context-member-identity`. Without a manifest the program is
open: an unseen importer could provide `TabsContext` or call `useTabs` and
replace the shared object's members. The local export list therefore keeps
the context open and `TabsList`'s read is not attributed to the signal.
