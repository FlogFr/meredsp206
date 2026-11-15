import sys, re, bisect, collections, pickle
lines=open(sys.argv[1]).read().splitlines()
starts=[]
for i in range(1,len(lines)):
    if lines[i].endswith('mov ebp, esp') and lines[i-1].endswith('push ebp'):
        starts.append(int(lines[i-1].split(':')[0],16))
addrs=[]
callers=collections.defaultdict(set)
for l in lines:
    m=re.match(r'([0-9a-f]+): call 0x([0-9a-f]+)$',l)
    if m:
        a=int(m.group(1),16); t=int(m.group(2),16)
        k=bisect.bisect_right(starts,a)-1
        if k>=0: callers[t].add(starts[k])
pickle.dump((starts,dict(callers)),open(sys.argv[2],'wb'))
