# A store as a prototype makes a local's Gets and writes the store's

ADR 0643, measured in Chrome: each warns `STRICT_READ_UNTRACKED` or `STORE_WRITE_OUTSIDE_SETTER`. No Get or write is followed
through the install, so each is uncertifiable at the install.
