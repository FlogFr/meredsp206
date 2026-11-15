# Binary Delphi/C++Builder DFM (TPF0) -> text converter
import sys, struct, os
class R:
    def __init__(s, b): s.b=b; s.p=0
    def u8(s): v=s.b[s.p]; s.p+=1; return v
    def i8(s): v=struct.unpack_from('<b',s.b,s.p)[0]; s.p+=1; return v
    def i16(s): v=struct.unpack_from('<h',s.b,s.p)[0]; s.p+=2; return v
    def i32(s): v=struct.unpack_from('<i',s.b,s.p)[0]; s.p+=4; return v
    def u32(s): v=struct.unpack_from('<I',s.b,s.p)[0]; s.p+=4; return v
    def i64(s): v=struct.unpack_from('<q',s.b,s.p)[0]; s.p+=8; return v
    def raw(s,n): v=s.b[s.p:s.p+n]; s.p+=n; return v
    def sstr(s): n=s.u8(); return s.raw(n).decode('latin1')
BIN_LIMIT = int(os.environ.get('BINLIM','64'))
def val(r, ind, out):
    t=r.u8()
    if t==0: return 'Null'
    if t==1: # list
        items=[]
        while r.b[r.p]!=0: items.append(val(r, ind+1, out))
        r.p+=1; return '(' + ' '.join(items) + ')'
    if t==2: return str(r.i8())
    if t==3: return str(r.i16())
    if t==4: return str(r.i32())
    if t==5: # 80-bit extended
        m=r.raw(10)
        mant=int.from_bytes(m[:8],'little'); se=int.from_bytes(m[8:],'little'); sign=-1 if se&0x8000 else 1; e=se&0x7fff
        return repr(sign*mant*2.0**(e-16383-63)) if e else '0'
    if t==6: return repr(r.sstr())
    if t==7: return r.sstr()
    if t==8: return 'False'
    if t==9: return 'True'
    if t==10:
        n=r.u32(); d=r.raw(n)
        return '{binary %d bytes: %s%s}'%(n, d[:BIN_LIMIT].hex(), '...' if n>BIN_LIMIT else '')
    if t==11:
        items=[]
        while True:
            x=r.sstr()
            if not x: break
            items.append(x)
        return '['+', '.join(items)+']'
    if t==12: n=r.u32(); return repr(r.raw(n).decode('latin1'))
    if t==13: return 'Nil'
    if t==14: # collection
        res=['<']
        while r.b[r.p]!=0:
            if r.b[r.p] in (2,3,4): val(r,ind,out)
            assert r.u8()==1
            res.append('  '*(ind+1)+'item')
            while r.b[r.p]!=0:
                nm=r.sstr(); res.append('  '*(ind+2)+nm+' = '+val(r,ind+2,out))
            r.p+=1
            res.append('  '*(ind+1)+'end')
        r.p+=1; res.append('  '*ind+'>')
        return '\n'.join(res)
    if t==15: return repr(struct.unpack_from('<f',r.raw(4))[0])
    if t==16: return 'Curr(%d)'%r.i64()
    if t==17: return 'Date(%r)'%struct.unpack_from('<d',r.raw(8))[0]
    if t==18:
        # the RS-232 help form obfuscates its RTF by adding 0x0E00 to each code unit
        n=r.u32(); x=r.raw(n*2).decode('utf-16le')
        if x and all(0xe00<=ord(c)<0xf00 for c in x[:8]): x=''.join(chr(ord(c)-0xe00) if 0xe00<=ord(c)<0xf00 else c for c in x)
        return repr(x)
    if t==19: return str(r.i64())
    if t==20: n=r.u32(); return repr(r.raw(n).decode('utf-8','replace'))
    if t==21: return repr(struct.unpack_from('<d',r.raw(8))[0])
    raise ValueError('unknown type %d at %d'%(t,r.p-1))
def obj(r, ind, out):
    flags=0
    if r.b[r.p]&0xf0==0xf0:
        flags=r.u8()&0x0f
        if flags&2: val(r,ind,out) # child position as typed int
    cls=r.sstr(); name=r.sstr()
    kw='inherited' if flags&1 else ('inline' if flags&4 else 'object')
    out.append('  '*ind+'%s %s: %s'%(kw,name,cls))
    while r.b[r.p]!=0:
        pn=r.sstr(); v=val(r,ind+1,out)
        out.append('  '*(ind+1)+pn+' = '+v)
    r.p+=1
    while r.b[r.p]!=0: obj(r,ind+1,out)
    r.p+=1
    out.append('  '*ind+'end')
b=open(sys.argv[1],'rb').read()
assert b[:4]==b'TPF0', b[:4]
r=R(b); r.p=4; out=[]
obj(r,0,out)
print('\n'.join(out))
