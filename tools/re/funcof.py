# find function start (nearest preceding 'push ebp; mov ebp, esp') for addresses
import sys,bisect
starts=[]
prev=None
for l in open(sys.argv[1]):
    a,_,ins=l.partition(': ')
    if ins.startswith('mov ebp, esp') and prev and prev[1].startswith('push ebp'): starts.append(int(prev[0],16))
    prev=(a,ins)
for x in sys.argv[2:]:
    v=int(x,16); i=bisect.bisect_right(starts,v)-1
    print('%s in func %08x'%(x,starts[i]))
