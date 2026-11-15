import sys, pickle, os, collections
starts,callers=pickle.load(open(sys.argv[1],'rb'))
callees=collections.defaultdict(set)
for t,cs in callers.items():
    for c in cs: callees[c].add(t)
names={}
for l in open(os.path.join(os.path.dirname(sys.argv[1]),'methods.txt')):
    a,n=l.split(); names[int(a,16)]=n
ctors={int(k,16):v for k,v in (x.split('=') for x in sys.argv[2].split(','))}
skip={0x417a14}
for f,n in sorted(names.items(), key=lambda x:x[1]):
    found=set(); seen=set(); frontier=[(f,0)]
    while frontier:
        g,d=frontier.pop()
        if g in seen or d>4: continue
        seen.add(g)
        for t in callees.get(g,()):
            if t in ctors: found.add(ctors[t])
            elif t not in skip and t not in names: frontier.append((t,d+1))
    if found: print('%-32s %s'%(n,' '.join(sorted(found))))
