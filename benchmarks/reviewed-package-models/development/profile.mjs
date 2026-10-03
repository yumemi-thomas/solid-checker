// Mutable working interface. Historical experiments remain immutable.
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
export const directory=dirname(fileURLToPath(import.meta.url));
export const historical=resolve(directory,'..');
export const defaults={
  hypothesis:'Authored pending probes and bounded synchronous projection preserve feedback while reducing the focused cycle time.',
  success:'Independent behavior/source audits pass for every selected case; old revisions stay refused; report measured time and sampled process memory.',
  cases:join(directory,'cases.mjs'),freeze:null,baseline:null,
  caseIds:['serial','concurrent'].flatMap(mode=>[
    `async-body-return-${mode}-adopted-child-object-deferred`,
    `async-body-return-${mode}-reaction-read-deferred`,
    `warning-accuracy-fresh-${mode}-named-allocation-open`,
    `warning-accuracy-fresh-${mode}-identity-control-open`,
  ]),
  tests:[join(directory,'development.test.mjs'),...['bounded-body-return-v2.test.mjs','late-lexical-continuation-v1.test.mjs','lexical-entry-map-v1.test.mjs','development-case-selection-v1.test.mjs'].map(name=>join(historical,name))],
};
