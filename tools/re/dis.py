# Disassemble a function (follows intra-function branches) with annotations
import pefile, sys, struct, capstone, re, os
EXE=os.environ.get('EXE')
pe=pefile.PE(EXE, fast_load=True); base=pe.OPTIONAL_HEADER.ImageBase
img=pe.get_memory_mapped_image()
names={}
for l in open(os.environ.get('METHODS', 'methods.txt')):
    a,n=l.split(); names[int(a,16)]=n
extra=os.environ.get('NAMES', 'names.txt')
if os.path.exists(extra):
    for l in open(extra):
        if l.strip() and not l.startswith('#'):
            a,n=l.split(None,1); names[int(a,16)]=n.strip()
md=capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32); md.detail=False
def rd(va,n): return img[va-base:va-base+n]
def strat(va):
    try:
        b=rd(va,200)
        # Delphi/C++B AnsiString literal: preceded by refcnt -1 & length; also try plain C string and wide string
        s=b.split(b'\0')[0]
        if len(s)>=3 and all(32<=c<127 for c in s): return repr(s.decode())
        if len(b)>4 and b[1]==0 and 32<=b[0]<127:
            w=b.decode('utf-16le','ignore').split('\0')[0]
            if len(w)>=3: return 'L'+repr(w)
    except Exception: pass
    return None
def dis(start, maxn=3000):
    todo=[start]; seen=set(); out={}
    while todo:
        a=todo.pop()
        while a not in seen and len(out)<maxn:
            seen.add(a)
            ins=next(md.disasm(rd(a,16),a),None)
            if not ins: break
            out[a]=ins
            m=ins.mnemonic
            tgt=None
            try: tgt=int(ins.op_str,16)
            except: pass
            if m.startswith('j') and tgt is not None:
                todo.append(tgt)
                if m=='jmp': break
            if m in('ret','retf') or (m=='jmp' and tgt is None): break
            a+=ins.size
    for a in sorted(out):
        ins=out[a]; ann=''
        for h in re.findall(r'0x[0-9a-f]{5,8}',ins.op_str):
            v=int(h,16)
            if v in names: ann+=' <'+names[v]+'>'
            elif base<=v<base+len(img):
                s=strat(v)
                if s: ann+=' '+s[:80]
        print('%08x: %-8s %s%s'%(a,ins.mnemonic,ins.op_str,('   ;'+ann) if ann else ''))
for x in sys.argv[1:]: 
    print('==== %s'%x); dis(int(x,16))
