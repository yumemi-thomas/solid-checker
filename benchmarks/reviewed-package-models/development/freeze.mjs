// Snapshot mutable tools once at a milestone; old snapshots never change.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {basename,join,resolve} from 'node:path';
import {activePins,authenticate} from './evidence.mjs';
import {hash,read} from '../catalog.mjs';
const path=resolve(process.argv[2]),archive=path+'.sources';assert(!existsSync(path)&&!existsSync(archive));
const previous=resolve('rust/target/lightpanda-detector-freeze-v2.json'),old=read(previous);
for(const pin of [...old.files,old.baseline])authenticate(pin);
mkdirSync(archive,{recursive:true});
const workingFiles=activePins().map(pin=>{
  const archivePath=join(archive,basename(pin.path));writeFileSync(archivePath,readFileSync(pin.path));
  return {...pin,archivePath};
});
const seal={authority:false,certification:false,frozenAt:new Date().toISOString(),baseline:{path:previous,sha256:hash(readFileSync(previous))},
  files:[...old.files,...workingFiles.map(pin=>({path:pin.archivePath,sha256:pin.sha256}))],workingFiles};
writeFileSync(path,JSON.stringify(seal,null,2)+'\n');console.log(JSON.stringify({path,workingFiles:workingFiles.length}));
