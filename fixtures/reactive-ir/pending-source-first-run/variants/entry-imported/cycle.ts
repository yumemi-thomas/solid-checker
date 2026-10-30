// Uncertifiable (in `App.tsx`): a project module imports the entry, so the
// entry may run before a module it imports has evaluated.
import "./main";
