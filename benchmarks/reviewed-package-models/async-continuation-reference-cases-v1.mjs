// New helper bodies after the reference-safe detector freeze; existing paired
// consumer setup is reused. These are helper-shape challenges, not new packages.
import previous from './async-continuation-cases-v1.mjs';
const pairs=previous.filter(row=>/^async-continuation-await-read-(target|control)$/.test(row.id));
const bodies=[
  ['assignment','const box={value:0};box.value=read();return box.value;'],
  ['compound-assignment','const box={value:0};box.value+=read();return box.value;'],
  ['post-increment','const box={value:0};box.value=read();return box.value++;'],
  ['delete','const box:{value?:number}={value:7};delete box.value;return read();'],
  ['tagged-receiver','const box={value:0,tag(parts:TemplateStringsArray){return this.value+parts.length-1;}};box.value=read();return box.tag`x`;'],
];
export default bodies.flatMap(([name,body])=>pairs.map(row=>({...row,id:'async-continuation-reference-'+name+'-'+row.stages[0].role,stages:[{...row.stages[0],helper:'export async function consume(read:()=>number){await Promise.resolve();'+body+'}'}]})));
