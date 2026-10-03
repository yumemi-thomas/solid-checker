// Deterministic integration replay with an unchanged config event before helper load.
import prior from './async-constant-cases-v2.mjs';
const control=prior.find(row=>row.id==='async-constant-unknown-loop-overridden-control');
if(!control)throw Error('Retained loading-race control is missing');
const cases=[];
for(const bad of [true,false]){
  const row=structuredClone(control);row.id='semantic-update-late-config-'+(bad?'target':'capture');row.duplicateConfigBeforeHelper=true;row.artifactOrigin='adapted-retained-consumer-with-forced-unchanged-config-event-and-real-source-edits';
  const normal='export async function consume(read:()=>number){await Promise.resolve();return read();}',changed='export async function consume(read:()=>number){await Promise.resolve();return read()+1;}';
  if(!bad){row.source=row.source.replace('const read=get;','const captured=get();const read=()=>captured;');}
  row.stages=[{id:'initial',helper:normal,initial:'1',afterUpdate:bad?'1':'2',desired:'2',role:bad?'target':'control'},{id:'helper-edit',helper:changed,initial:'2',afterUpdate:bad?'2':'3',desired:'3',role:bad?'target':'control'},{id:'comment-edit',helper:changed,mainComment:'\n// observed consumer edit\n',initial:'2',afterUpdate:bad?'2':'3',desired:'3',role:bad?'target':'control'}];cases.push(row);
}
const row=structuredClone(control);row.id='semantic-update-constant-proof-control';row.duplicateConfigBeforeHelper=true;row.artifactOrigin='adapted-original-quiet-constant-control-with-forced-unchanged-config-event';cases.push(row);
export default cases;
