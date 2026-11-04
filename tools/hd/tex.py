import struct, numpy as np
def chunks(b):
    o=0; out=[]
    while o+16<=len(b):
        v=struct.unpack_from('<I',b,o+4)[0]; k=v&0x7f; sz=((v>>7)&0x7ffff)*16
        if sz<16: break
        out.append((b[o:o+4],k,b[o+16:o+sz],o)); o+=sz
    return out
def c565(c): return np.array([((c>>11)&31)*255/31,((c>>5)&63)*255/63,(c&31)*255/31])
def decode(d):
    """DXT3 body -> HxWx4 float (alpha as stored, FFXI half scale)."""
    assert d[0]==0xa1 and d[0x39:0x3d]==b'3TXD'
    w,h=struct.unpack_from('<II',d,0x15); img=np.zeros((h,w,4)); o=0x45
    for by in range(h//4):
        for bx in range(w//4):
            a=d[o:o+8]; c0,c1,idx=struct.unpack_from('<HHI',d,o+8); e0,e1=c565(c0),c565(c1)
            pal=[e0,e1,(2*e0+e1)/3,(e0+2*e1)/3] if c0>c1 else [e0,e1,(e0+e1)/2,np.zeros(3)]
            for i in range(16):
                y,x=by*4+i//4,bx*4+i%4
                img[y,x,:3]=pal[(idx>>(2*i))&3]
                nib=(a[i//2]>>(4*(i%2)))&15
                img[y,x,3]=nib*17
            o+=16
    return img
def textures(path):
    b=open(path,'rb').read()
    return [(c[2][1:17].decode('latin1'),decode(c[2])) for c in chunks(b) if c[1]==0x20 and c[2][0]==0xa1]
