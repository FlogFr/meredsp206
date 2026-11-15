# For each call to target, print the preceding push instructions (cdecl args, arg0 last)
import sys, re, bisect
lines=open(sys.argv[1]).read().splitlines()
targets=set(sys.argv[2:])
starts=[]
for i in range(1,len(lines)):
    if lines[i].endswith('mov ebp, esp') and lines[i-1].endswith('push ebp'): starts.append(i-1)
for i,l in enumerate(lines):
    m=re.match(r'([0-9a-f]+): call 0x([0-9a-f]+)$',l)
    if m and m.group(2).zfill(8) in targets:
        j=i-1; pushes=[]; ctx=[]
        while j>i-40 and len(pushes)<8:
            ins=lines[j].split(': ',1)[1]
            if ins.startswith('push '): pushes.append(ins[5:])
            if ins.startswith('call') or ins.startswith('ret'): break
            ctx.append(ins); j-=1
        k=bisect.bisect_right(starts,i)-1
        fs=lines[starts[k]].split(':')[0]
        # find movs feeding pushed regs
        print('%s func %s -> %s args: %s'%(m.group(1),fs,m.group(2),' | '.join(pushes)))
