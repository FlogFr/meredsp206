import pefile, sys, os
pe = pefile.PE(sys.argv[1])
out = sys.argv[2]
types = {v:k for k,v in pefile.RESOURCE_TYPE.items()}
for t in pe.DIRECTORY_ENTRY_RESOURCE.entries:
    tname = str(t.name) if t.name else types.get(t.id, str(t.id))
    for n in t.directory.entries:
        nname = str(n.name) if n.name else str(n.id)
        for l in n.directory.entries:
            d = l.data.struct
            data = pe.get_data(d.OffsetToData, d.Size)
            p = os.path.join(out, tname)
            os.makedirs(p, exist_ok=True)
            fn = os.path.join(p, nname.replace('/','_'))
            open(fn,'wb').write(data)
            print(tname, nname, len(data))
