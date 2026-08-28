# Asset import is not executable package closure

`index.js` imports `./styles.css`. Neither the authoritative TypeScript module
graph nor the package runtime/declaration resolver identifies it as an
executable module for this artifact case. The exact closure therefore contains
the JavaScript module without inventing package semantics for the stylesheet.

This is the negative control for unresolved imports that can select real
runtime modules. The export still carries six local unresolved claim leaves and
four proof candidates; dropping the asset from executable closure does not
close any semantic domain.
