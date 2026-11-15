# Find published methods (VCL method tables): [u16 size][u32 code][shortstring name]
import pefile, sys, re, struct
pe=pefile.PE(sys.argv[1], fast_load=True)
data=open(sys.argv[1],'rb').read()
base=pe.OPTIONAL_HEADER.ImageBase
def off2va(o):
    for s in pe.sections:
        if s.PointerToRawData<=o<s.PointerToRawData+s.SizeOfRawData: return base+s.VirtualAddress+o-s.PointerToRawData
text=[s for s in pe.sections if s.Name.startswith(b'.text')][0]
lo=base+text.VirtualAddress; hi=lo+text.Misc_VirtualSize
res={}
for m in re.finditer(rb'[\x03-\x40]([A-Za-z_][A-Za-z0-9_]{2,63})', data):
    n=m.group(0)[0]; name=m.group(1)
    if len(name)!=n: continue
    o=m.start()
    if o<6: continue
    sz,code=struct.unpack_from('<HI',data,o-6)
    if lo<=code<hi and sz==6+1+n:
        res[name.decode()]=code
for k,v in sorted(res.items(), key=lambda x:x[1]): print('%08x %s'%(v,k))
