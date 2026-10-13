// New source variants authored after the refined revision/cache detector froze.
import prior from './feedback-revision-cases-v1.mjs';
const cases=[];
for(const original of prior.filter(row=>row.package==='@solid-primitives/map')){
  const source=original.source.replace("import {consume} from './consumer';","import * as helpers from './consumer';").replace('const invoke=consume;','').replaceAll('invoke(get)','helpers.consume(get)');
  cases.push({...original,id:'namespace-'+original.id,source});
}
for(const original of prior.filter(row=>row.package==='@solid-primitives/controlled-signal')){
  const source=original.source.replace("import {consume} from './consumer';","import {consume} from './bridge';");
  cases.push({...original,id:'reexport-'+original.id,source,files:{'bridge.ts':"export {consume} from './consumer';"}});
}
const late=prior.find(row=>row.id==='controlled-signal-stale');
const stages=late.stages.map(stage=>({...stage,helper:stage.helper.replace('export function consume','export async function consume').replace('{return read();}','{await Promise.resolve();return read();}').replace('{read();return 9;}','{await Promise.resolve();read();return 9;}').replace('{const wrong:boolean=1;return read();}','{const wrong:boolean=1;await Promise.resolve();return read();}')}));
cases.push({...late,id:'delayed-'+late.id,stages});
export default cases;
