import sys, pickle, os
starts,callers=pickle.load(open(sys.argv[1],'rb'))
names={}
for l in open(os.path.join(os.path.dirname(sys.argv[1]),'methods.txt')):
    a,n=l.split(); names[int(a,16)]=n
def up(f, depth, seen):
    res=set()
    if f in names: return {names[f]}
    if depth==0: return {'?%x'%f}
    for c in callers.get(f,()):
        if c in seen: continue
        res|=up(c,depth-1,seen|{c})
    return res or {'?%x(root)'%f}
for x in sys.argv[2:]:
    f=int(x,16)
    r=up(f,int(os.environ.get('DEPTH','5')),{f})
    print(x, ' '.join(sorted(r)[:25]))
