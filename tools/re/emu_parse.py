# Emulate the 0x24 page-blob parser core (0x419d8b..0x41aaab) to map blob offsets -> model offsets
import pefile, sys, struct
from unicorn import *
from unicorn.x86_const import *
EXE=sys.argv[1]; START=int(sys.argv[2],16); END=int(sys.argv[3],16)
RAW=int(sys.argv[4],16)      # raw blob offset inside model (e.g. 0x3c6)
IDX0=int(sys.argv[5])        # initial index value at [ebp-0x34]
NRAW=int(sys.argv[6])
pe=pefile.PE(EXE, fast_load=True); base=pe.OPTIONAL_HEADER.ImageBase
img=pe.get_memory_mapped_image()
MODEL=0x20000000; MSIZE=0x10000; STK=0x30000000
def run(fill):
    mu=Uc(UC_ARCH_X86, UC_MODE_32)
    size=(len(img)+0xfff)&~0xfff
    mu.mem_map(base,size); mu.mem_write(base,img)
    mu.mem_map(MODEL,MSIZE); mu.mem_map(STK-0x10000,0x20000)
    model=bytearray(MSIZE)
    for i in range(NRAW): model[RAW+i]=fill(i)
    mu.mem_write(MODEL,bytes(model))
    ebp=STK
    mu.mem_write(ebp+8,struct.pack('<I',MODEL))
    mu.mem_write(ebp-0x34,struct.pack('<i',IDX0))
    mu.reg_write(UC_X86_REG_EBP,ebp); mu.reg_write(UC_X86_REG_ESP,ebp-0x100)
    mu.emu_start(START,END,count=2000000)
    return bytes(mu.mem_read(MODEL,MSIZE))
A=run(lambda i:(i&0xff))
B=run(lambda i:((i>>8)+1))
Z=run(lambda i:0)
out=[]
for off in range(MSIZE):
    if RAW<=off<RAW+NRAW: continue
    if A[off]!=Z[off] or B[off]!=Z[off]:
        hi=B[off]-1; lo=A[off]
        out.append((off, hi*256+lo, A[off], B[off]))
for off,idx,a,b in out:
    print('model+0x%04x <- blob[%d]'%(off,idx) + ('' if 0<=idx<NRAW else '  (?? a=%02x b=%02x)'%(a,b)))
