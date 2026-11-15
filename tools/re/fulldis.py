import pefile, sys, capstone
pe=pefile.PE(sys.argv[1], fast_load=True); base=pe.OPTIONAL_HEADER.ImageBase
t=[s for s in pe.sections if s.Name.startswith(b'.text')][0]
code=t.get_data(); va=base+t.VirtualAddress
md=capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32); md.skipdata=True
with open(sys.argv[2],'w') as f:
    for i in md.disasm(code, va):
        f.write('%08x: %s %s\n'%(i.address,i.mnemonic,i.op_str))
