// Preserve the original sources, labels and flows in a separate guard profile.
import cases from './family-holdout-cases.mjs';
export default cases.filter(row => row.package === '@solid-primitives/mouse');
