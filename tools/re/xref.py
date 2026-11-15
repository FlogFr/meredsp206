# find raw dword references to given VAs in all sections
import pefile, sys, struct
pe=pefile.PE(sys.argv[1], fast_load=True); base=pe.OPTIONAL_HEADER.ImageBase
img=pe.get_memory_mapped_image()
for x in sys.argv[2:]:
    v=struct.pack('<I',int(x,16)); i=0; hits=[]
    while True:
        i=img.find(v,i)
        if i<0: break
        hits.append('%08x'%(base+i)); i+=1
    print(x, hits[:20])
