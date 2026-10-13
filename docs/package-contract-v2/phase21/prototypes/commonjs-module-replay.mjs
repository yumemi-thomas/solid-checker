// Diagnostic simulated-host interpreter. No result is certification evidence.
import { createRequire } from 'node:module';
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
import path from 'node:path';
const require = createRequire(import.meta.url);
const { parse } = createRequire(new URL('../../../../packages/cli/package.json', import.meta.url))('acorn');
const missing = Symbol('missing');
const object = prototype => ({ kind: 'object', prototype, slots: new Map() });
const data = (value, enumerable = true) => ({ value, writable: true, enumerable, configurable: true });
const intrinsic = name => ({ kind: 'intrinsic', name });
const fail = (node, why) => { throw new Error(`${node?.start}:${node?.end}: ${why}`); };

function simulate(sources) {
  let fuel = 20000, depth = 0;
  const steps = [], modules = new Map(), hostRequirements=[];
  const proto = object(null);
  proto.host='Object.prototype';
  proto.slots.set('hasOwnProperty', data(intrinsic('hasOwnProperty'), false));
  const ObjectValue = object(proto);
  ObjectValue.host='Object';
  for (const name of ['create', 'defineProperty', 'getOwnPropertyDescriptor']) ObjectValue.slots.set(name, data(intrinsic(name), false));
  ObjectValue.slots.set('prototype', {value:proto,writable:false,enumerable:false,configurable:false});
  const ErrorValue = intrinsic('Error');
  const globals = { values: new Map([['Object',ObjectValue],['Error',ErrorValue],['undefined',undefined]]), constants:new Set(['undefined']), parent:null };
  function requireHost(node,requirement){
    hostRequirements.push({module:node.sourceFile,start:node.start,end:node.end,...requirement});
  }
  function tick(node) { if (--fuel < 0) fail(node, 'fuel limit'); }
  function lookup(obj, key, node) {
    if (obj === null) return missing;
    if (obj?.kind !== 'object') fail(node, 'unknown object or prototype');
    if(obj.host){
      const descriptor=obj.slots.get(key);
      requireHost(node,{kind:'property-descriptor',host:obj.host,key,present:!!descriptor,
        ...(descriptor?{descriptorKind:'value' in descriptor?'data':'accessor',writable:descriptor.writable,enumerable:descriptor.enumerable,configurable:descriptor.configurable}:{}),
        ...(descriptor?.value?.kind==='intrinsic'?{intrinsic:descriptor.value.name}:{}),
        ...(descriptor?.value?.host?{valueHost:descriptor.value.host}:{})});
      if(!descriptor)requireHost(node,{kind:'prototype',host:obj.host,prototype:obj.prototype?.host??null});
    }
    if (obj.slots.has(key)) return obj.slots.get(key);
    return lookup(obj.prototype, key, node);
  }
  function read(obj, key, node) {
    if (obj?.kind === 'intrinsic' && key === 'call') {
      requireHost(node,{kind:'intrinsic',host:'Function.prototype.call'});
      return { kind: 'intrinsicCall', target: obj };
    }
    const slot = lookup(obj, key, node);
    if (slot === missing) return undefined;
    if ('get' in slot) return slot.get === undefined ? undefined : call(slot.get, [], obj, node);
    return slot.value;
  }
  function write(obj, key, value, node) {
    if (obj?.kind !== 'object') fail(node, 'unknown write object');
    const previous = lookup(obj, key, node);
    if (previous !== missing && (!('value' in previous) || !previous.writable)) fail(node, 'unproved write');
    const own=obj.slots.get(key);
    obj.slots.set(key, own ? {...own,value} : data(value));
    steps.push({ module: node.sourceFile, start: node.start, end: node.end, operation: 'write', key });
    return value;
  }
  function binding(env, key, node) {
    for (let next = env; next; next = next.parent) if (next.values.has(key)) return next;
    fail(node, `unbound identifier ${key}`);
  }
  function getBinding(env,key,node) {
    const owner=binding(env,key,node);
    if(owner.uninitialized?.has(key))fail(node,'temporal dead zone');
    if(owner===globals)requireHost(node,{kind:'global-binding',name:key,valueIdentity:key});
    return owner.values.get(key);
  }
  function setBinding(env,key,value,node,initialization=false) {
    const owner=binding(env,key,node);
    if(!initialization&&(owner.uninitialized?.has(key)||owner.constants?.has(key)))fail(node,'invalid lexical assignment');
    owner.uninitialized?.delete(key);owner.values.set(key,value);return value;
  }
  function member(node, env) {
    if (node.optional) fail(node, 'optional member');
    const obj = evaluate(node.object, env);
    const key = node.computed ? evaluate(node.property, env) : node.property.name;
    if (typeof key !== 'string') fail(node, 'nonstring key');
    return [obj, key];
  }
  function callable(node, env) { return { kind: 'function', node, env, module: env.module }; }
  function evaluate(node, env) {
    tick(node);
    if (++depth > 96) fail(node, 'depth limit');
    try { return expression(node, env); } finally { depth--; }
  }
  function expression(node, env) {
    switch (node.type) {
      case 'Literal': if (node.regex || typeof node.value === 'bigint') fail(node, 'literal'); return node.value;
      case 'Identifier': return getBinding(env, node.name, node);
      case 'ThisExpression': return env.thisValue;
      case 'FunctionExpression': case 'ArrowFunctionExpression': return callable(node, env);
      case 'MemberExpression': { const [obj, key] = member(node, env); return read(obj, key, node); }
      case 'UnaryExpression': {
        const value = evaluate(node.argument, env);
        if (node.operator === 'void') return undefined;
        if (node.operator === '!') return !value;
        fail(node, 'unary operation'); break;
      }
      case 'LogicalExpression': {
        const left = evaluate(node.left, env);
        if (node.operator === '&&') return left ? evaluate(node.right, env) : left;
        if (node.operator === '||') return left ? left : evaluate(node.right, env);
        fail(node, 'logical operation'); break;
      }
      case 'ConditionalExpression': return evaluate(evaluate(node.test, env) ? node.consequent : node.alternate, env);
      case 'BinaryExpression': {
        const left = evaluate(node.left, env), right = evaluate(node.right, env);
        if (node.operator === '===') return left === right;
        if (node.operator === '!==') return left !== right;
        if (node.operator === '|' && typeof left === 'number' && typeof right === 'number') return left | right;
        if (node.operator === 'in' && typeof left === 'string') return lookup(right, left, node) !== missing;
        fail(node, 'unproved binary operation'); break;
      }
      case 'AssignmentExpression': {
        if (node.operator !== '=') fail(node, 'compound assignment');
        if (node.left.type === 'Identifier') {
          binding(env,node.left.name,node);
          const value = evaluate(node.right, env); return setBinding(env,node.left.name,value,node);
        }
        if (node.left.type !== 'MemberExpression') fail(node, 'assignment target');
        const [obj, key] = member(node.left, env); return write(obj, key, evaluate(node.right, env), node);
      }
      case 'ObjectExpression': {
        const obj = object(proto);
        for (const p of node.properties) {
          if (p.type !== 'Property' || p.computed || p.method || p.kind !== 'init' || p.shorthand) fail(p, 'record property');
          const key = p.key.name ?? p.key.value;
          if (typeof key !== 'string' || key === '__proto__') fail(p, 'record key');
          obj.slots.set(key, data(evaluate(p.value, env)));
        }
        return obj;
      }
      case 'CallExpression': {
        let fn, receiver;
        if (node.callee.type === 'MemberExpression') {
          const [obj, key] = member(node.callee, env); receiver = obj; fn = read(obj, key, node);
        } else fn = evaluate(node.callee, env);
        if (node.optional || node.arguments.some(arg => arg.type === 'SpreadElement')) fail(node, 'call form');
        return call(fn, node.arguments.map(arg => evaluate(arg, env)), receiver, node);
      }
      default: fail(node, `expression ${node.type}`);
    }
  }
  function call(fn, args, receiver, node) {
    tick(node);
    if (fn?.kind === 'require') {
      if (args.length !== 1 || typeof args[0] !== 'string' || args[0] !== './options') fail(node, 'unknown require');
      return load('options.js');
    }
    if (fn?.kind === 'intrinsicCall') return call(fn.target, args.slice(1), args[0], node);
    if (fn?.kind === 'intrinsic') {
      const [obj, key, desc] = args;
      switch (fn.name) {
        case 'hasOwnProperty':
          if (receiver?.kind !== 'object' || typeof obj !== 'string') fail(node, 'hasOwnProperty operands');
          return receiver.slots.has(obj);
        case 'getOwnPropertyDescriptor': {
          if (obj?.kind !== 'object' || typeof key !== 'string') fail(node, 'descriptor operands');
          if (!obj.slots.has(key)) return undefined;
          const result = object(proto);
          for (const [k,v] of Object.entries(obj.slots.get(key))) result.slots.set(k, data(v));
          return result;
        }
        case 'defineProperty': {
          if (obj?.kind !== 'object' || typeof key !== 'string' || desc?.kind !== 'object') fail(node, 'define operands');
          const previous = obj.slots.get(key);
          if (previous && !previous.configurable) fail(node, 'nonconfigurable redefinition');
          const fields = {};
          for (const k of ['enumerable','configurable','value','writable','get','set']) {
            if (lookup(desc,k,node) !== missing) fields[k] = read(desc,k,node);
          }
          if (('get' in fields || 'set' in fields) && ('value' in fields || 'writable' in fields)) fail(node, 'mixed descriptor');
          if ('set' in fields) fail(node, 'setter descriptor');
          if ('get' in fields && fields.get !== undefined && fields.get?.kind !== 'function') fail(node, 'unknown getter');
          // This prototype supports new properties and the target's data-to-
          // accessor conversion only; it refuses other partial redefinitions.
          if (previous && !('get' in fields)) fail(node, 'partial redefinition');
          const enumerable='enumerable' in fields?!!fields.enumerable:previous?.enumerable??false;
          const configurable='configurable' in fields?!!fields.configurable:previous?.configurable??false;
          obj.slots.set(key, 'get' in fields ? { get: fields.get, enumerable, configurable } :
            { value: fields.value, writable: !!fields.writable, enumerable: !!fields.enumerable, configurable: !!fields.configurable });
          steps.push({ module: node.sourceFile, start: node.start, end: node.end, operation: 'define', key }); return obj;
        }
        default: fail(node, `unmodeled intrinsic ${fn.name}`);
      }
    }
    if (fn?.kind !== 'function' || fn.node.async || fn.node.generator || fn.node.type === 'ArrowFunctionExpression') fail(node, 'unknown callable');
    if (fn.node.params.some(p => p.type !== 'Identifier')) fail(node, 'parameter form');
    const env = { values: new Map(), parent: fn.env, module: fn.module, thisValue: receiver };
    fn.node.params.forEach((p,i) => env.values.set(p.name,args[i]));
    if (fn.node.body.type !== 'BlockStatement') return evaluate(fn.node.body,env);
    const result = executeList(fn.node.body.body,env);
    return result?.value;
  }
  function executeList(statements,env) {
    if (statements.length > 256) throw new Error('statement bound');
    env.uninitialized??=new Set();env.constants??=new Set();
    for (const s of statements) {
      if (s.type === 'FunctionDeclaration') {
        if(env.values.has(s.id.name))fail(s,'duplicate function binding');
        env.values.set(s.id.name,callable(s,env));
      }
      if (s.type === 'VariableDeclaration') for (const d of s.declarations) {
        if (d.id.type !== 'Identifier' || env.values.has(d.id.name)) fail(d,'duplicate or destructured binding');
        env.values.set(d.id.name,undefined);
        if(s.kind!=='var')env.uninitialized.add(d.id.name);
        if(s.kind==='const')env.constants.add(d.id.name);
      }
    }
    for (const s of statements) { const result=statement(s,env); if(result) return result; }
  }
  function statement(node,env) {
    tick(node);
    switch(node.type) {
      case 'EmptyStatement': case 'FunctionDeclaration': return;
      case 'ExpressionStatement': evaluate(node.expression,env);return;
      case 'VariableDeclaration':
        for(const d of node.declarations) setBinding(env,d.id.name,d.init?evaluate(d.init,env):undefined,d,true); return;
      case 'BlockStatement':
        if(node.body.some(s=>s.type==='VariableDeclaration'&&s.kind!=='var'))fail(node,'block lexical declarations require scope model');
        return executeList(node.body,env);
      case 'IfStatement': return evaluate(node.test,env) ? statement(node.consequent,env) : node.alternate ? statement(node.alternate,env) : undefined;
      case 'ReturnStatement': return {value:node.argument?evaluate(node.argument,env):undefined};
      case 'ForInStatement': {
        if(node.left.type!=='VariableDeclaration'||node.left.kind!=='var'||node.left.declarations.length!==1||node.left.declarations[0].id.type!=='Identifier')fail(node,'loop binding');
        const name=node.left.declarations[0].id.name,obj=evaluate(node.right,env),keys=[],seen=new Set();
        for(let next=obj;next!==null;next=next.prototype){
          if(next?.kind!=='object')fail(node,'unknown enumerable prototype');
          for(const [key,slot]of next.slots){if(!seen.has(key)&&slot.enumerable)keys.push(key);seen.add(key);}
        }
        if(keys.length>256)fail(node,'enumeration bound');
        env.values.set(name,undefined);
        for(const key of keys){env.values.set(name,key);const result=statement(node.body,env);if(result)return result;}
        return;
      }
      case 'ClassDeclaration':
        if(node.body.body.length!==0||evaluate(node.superClass,env)!==ErrorValue)fail(node,'unmodeled class initialization');
        if(env.values.has(node.id.name))fail(node,'duplicate class binding');
        env.values.set(node.id.name,{kind:'class',module:env.module,start:node.start,end:node.end});return;
      default:fail(node,`statement ${node.type}`);
    }
  }
  function load(name) {
    if(modules.has(name)){const saved=modules.get(name);if(!saved.done)throw new Error('cyclic initialization');return saved.exports;}
    if(!sources.has(name))throw new Error('missing module');
    const source=sources.get(name);if(Buffer.byteLength(source)>65536)throw new Error('source bound');
    const exports=object(proto);modules.set(name,{exports,done:false});
    const env={values:new Map([['exports',exports],['require',{kind:'require'}]]),parent:globals,module:name,thisValue:exports};
    const ast=parse(source,{ecmaVersion:2023,sourceType:'script'});
    let nodes=0;
    function tag(value){
      if(!value||typeof value!=='object')return;
      if(++nodes>10000)throw new Error('AST node bound');
      if(typeof value.type==='string'){
        value.sourceFile=name;
        value.start=Buffer.byteLength(source.slice(0,value.start));
        value.end=Buffer.byteLength(source.slice(0,value.end));
      }
      for(const child of Object.values(value))if(typeof child==='object')tag(child);
    }
    tag(ast);
    executeList(ast.body,env);
    modules.get(name).done=true;return exports;
  }
  const root=load('index.js');
  function materialize(value,level=0){
    if(level>8)throw new Error('materialization depth');
    if(value?.kind==='function')return {kind:'function',module:value.module,start:value.node.start,end:value.node.end};
    if(value?.kind==='class')return value;
    if(value?.kind==='object')return Object.fromEntries([...value.slots].map(([k])=>[k,materialize(read(value,k,{start:0,end:0}),level+1)]));
    return value;
  }
  const values=materialize(root),aliases={parseIsParseJSON:read(root,'parse',{})===read(root,'parseJSON',{})};
  return {values,aliases,steps,hostRequirements,remainingFuel:fuel};
}

if (!process.argv[2]) throw new Error('Pass the retained full-report JSON path');
const reportBytes=readFileSync(process.argv[2]);
const report=JSON.parse(reportBytes);
const row=report.results.find(row=>row.probeId==='@tanstack/ai-solid@0.19.1|solid1|only');
if(!row?.retainedArtifacts?.projectDir)throw new Error('Report lacks the retained target project');
const parentManifest=path.join(row.retainedArtifacts.projectDir,'node_modules/@tanstack/ai-solid/package.json');
const parent=JSON.parse(readFileSync(parentManifest));
if(parent.name!=='@tanstack/ai-solid'||parent.version!=='0.19.1')throw new Error('Unexpected parent identity');
const entry=createRequire(parentManifest).resolve('partial-json');
const root=path.dirname(entry)+path.sep;
const manifest=JSON.parse(readFileSync(path.join(root,'../package.json')));
if(manifest.name!=='partial-json'||manifest.version!=='0.1.7')throw new Error('Unexpected dependency identity');
const sources=new Map(['options.js','index.js'].map(name=>[name,readFileSync(root+name,'utf8')]));
const result=simulate(sources);
// Captured before loading package code; not an authenticated host session.
const hostObjects=new Map([['Object',Object],['Object.prototype',Object.prototype]]);
const hostIntrinsics=new Map([['create',Object.create],['defineProperty',Object.defineProperty],
  ['getOwnPropertyDescriptor',Object.getOwnPropertyDescriptor],['hasOwnProperty',Object.prototype.hasOwnProperty],
  ['Function.prototype.call',Function.prototype.call]]);
const hostGlobals=new Map([['Object',Object],['Error',Error],['undefined',undefined]]);
const capturedRequirements=result.hostRequirements.map(requirement=>{
  switch(requirement.kind){
    case 'global-binding':return {kind:requirement.kind,name:requirement.name,value:globalThis[requirement.name]};
    case 'intrinsic':return {kind:requirement.kind,host:requirement.host,value:Function.prototype.call};
    case 'prototype':return {kind:requirement.kind,host:requirement.host,value:Object.getPrototypeOf(hostObjects.get(requirement.host))};
    case 'property-descriptor':return {kind:requirement.kind,host:requirement.host,key:requirement.key,descriptor:Object.getOwnPropertyDescriptor(hostObjects.get(requirement.host),requirement.key)};
    default:throw new Error('Unknown requirement kind');
  }
});
function matchesCapturedHost(observations){
  if(observations.length!==result.hostRequirements.length)return false;
  return result.hostRequirements.every((requirement,i)=>{
    const observed=observations[i];
    if(!observed||observed.kind!==requirement.kind||observed.host!==requirement.host||observed.name!==requirement.name||observed.key!==requirement.key)return false;
    switch(requirement.kind){
      case 'global-binding':return hostGlobals.has(requirement.valueIdentity)&&observed.value===hostGlobals.get(requirement.valueIdentity);
      case 'intrinsic':return hostIntrinsics.has(requirement.host)&&observed.value===hostIntrinsics.get(requirement.host);
      case 'prototype':return observed.value===(requirement.prototype===null?null:hostObjects.get(requirement.prototype));
      case 'property-descriptor':{
        const d=observed.descriptor;
        if(!requirement.present)return d===undefined;
        return d!==undefined&&('value' in d?'data':'accessor')===requirement.descriptorKind&&d.writable===requirement.writable&&d.enumerable===requirement.enumerable&&d.configurable===requirement.configurable&&
          (!requirement.intrinsic||(hostIntrinsics.has(requirement.intrinsic)&&d.value===hostIntrinsics.get(requirement.intrinsic)))&&
          (!requirement.valueHost||(hostObjects.has(requirement.valueHost)&&d.value===hostObjects.get(requirement.valueHost)));
      }
      default:return false;
    }
  });
}
assert.equal(matchesCapturedHost(capturedRequirements),true,JSON.stringify(result.hostRequirements.flatMap((r,i)=>{
  if(r.kind!=='property-descriptor'||!r.present)return [];
  const d=capturedRequirements[i].descriptor;
  return !d||d.writable!==r.writable||d.enumerable!==r.enumerable||d.configurable!==r.configurable?[{requirement:r,observed:d}]:[];
}).slice(0,2)));
assert.equal(matchesCapturedHost(capturedRequirements.slice(1)),false);
const altered=capturedRequirements.map(row=>({...row}));
const absent=altered.findIndex(row=>row.kind==='property-descriptor'&&row.descriptor===undefined);
assert.ok(absent>=0);
altered[absent].descriptor={value:1,writable:true,configurable:true,enumerable:true};
assert.equal(matchesCapturedHost(altered),false);
const changedIntrinsic=capturedRequirements.map(row=>({...row}));
const intrinsicIndex=changedIntrinsic.findIndex(row=>row.kind==='intrinsic');
assert.ok(intrinsicIndex>=0);changedIntrinsic[intrinsicIndex].value=function replacement(){};
assert.equal(matchesCapturedHost(changedIntrinsic),false);
const actual=require(root+'index.js');
assert.deepEqual(Object.keys(result.values).sort(),Object.getOwnPropertyNames(actual).sort());
for(const [key,value]of Object.entries(result.values)){
  if(value?.kind==='function'||value?.kind==='class')assert.equal(typeof actual[key],'function');
  else assert.deepEqual(value,actual[key]);
}
assert.equal(result.aliases.parseIsParseJSON,true);
const control=source=>new Map([['index.js',source]]);
assert.throws(()=>simulate(control('exports.bad = later; const later = 1;')),/temporal dead zone/);
assert.throws(()=>simulate(control('const key=1; key=2;')),/invalid lexical assignment/);
assert.throws(()=>simulate(control('{ let key=1; } exports.bad=key;')),/block lexical declarations require scope model/);
assert.throws(()=>simulate(control('require("./missing");')),/unknown require/);
assert.throws(()=>simulate(new Map([['index.js','require("./options");'],['options.js','require("./options");']])),/cyclic initialization/);
assert.throws(()=>simulate(control('exports.bad=eval("1");')),/unbound identifier eval/);
assert.throws(()=>simulate(new Map([['index.js','Object={}; require("./options");'],['options.js',sources.get('options.js')]])),/unknown callable/);
assert.equal(simulate(new Map([['index.js','const Object={}; exports.value=require("./options").ALL;'],['options.js',sources.get('options.js')]])).values.value,511);
assert.equal(simulate(control('Object.defineProperty(exports,"value",{value:1,writable:true}); exports.value=2; exports.enumerable=Object.getOwnPropertyDescriptor(exports,"value").enumerable;')).values.enumerable,false);
assert.equal(simulate(control('exports.value=1; Object.defineProperty(exports,"value",{get:function(){return 2;}}); exports.configurable=Object.getOwnPropertyDescriptor(exports,"value").configurable;')).values.configurable,true);
const changed=simulate(control('exports.parse=function first(){}; exports.parseJSON=function second(){};'));
assert.equal(changed.aliases.parseIsParseJSON,false);
console.log(JSON.stringify({diagnostic:true,grantsReceiptAuthority:false,hypotheticalHost:true,
  reportSha256:createHash('sha256').update(reportBytes).digest('hex'),probeId:row.probeId,entry,
  sourceDigests:Object.fromEntries([...sources].map(([name,source])=>[name,createHash('sha256').update(source).digest('hex')])),
  runtimeSurfaceComparisonMatched:true,hostObservationMatched:true,hostMutationControls:3,
  negativeControls:7,propertyAndGlobalControls:3,distinctFunctionControl:true,...result},null,2));
