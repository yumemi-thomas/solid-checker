import json,os,sys,collections
before,after=sys.argv[1],sys.argv[2]
for name in sorted(os.listdir(after)):
  if not name.endswith('.json'): continue
  key=lambda f:(f['rule'],f['kind'],f.get('blocker'),f['path'],f['start'],f['end'])
  a=[key(f) for f in json.load(open(os.path.join(after,name)))['findings']]
  b=[key(f) for f in json.load(open(os.path.join(before,name)))['findings']] if os.path.exists(os.path.join(before,name)) else []
  ca,cb=collections.Counter(a),collections.Counter(b)
  add=list((ca-cb).elements()); rem=list((cb-ca).elements())
  if not add and not rem: continue
  print('==',name)
  for tag,xs in (('-',rem),('+',add)):
    for r in sorted(xs,key=lambda r:(r[3],r[4])):
      try: src=open(r[3]).read()
      except: src=''
      line=src[:r[4]].count('\n')+1
      ls=src.split('\n')[line-1].strip() if src else ''
      print(f'  {tag} {r[2]} {r[3].split("/")[-1]}:{line} `{src[r[4]:r[5]][:40]}`  | {ls[:110]}')
