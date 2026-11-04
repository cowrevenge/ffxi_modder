import struct, sys, numpy as np, matplotlib
matplotlib.use('Agg'); import matplotlib.pyplot as plt
from mpl_toolkits.mplot3d.art3d import Poly3DCollection

def chunks(b):
    o=0; out=[]
    while o+16<=len(b):
        v=struct.unpack_from('<I',b,o+4)[0]; k=v&0x7f; sz=((v>>7)&0x7ffff)*16
        if sz<16: break
        out.append((b[o:o+4],k,b[o+16:o+sz])); o+=sz
    return out

def c565(c): return np.array([((c>>11)&31)*255/31,((c>>5)&63)*255/63,(c&31)*255/31])
def dxt3(d):
    w,h=struct.unpack_from('<II',d,0x15); img=np.zeros((h,w,3)); o=0x45
    for by in range(h//4):
        for bx in range(w//4):
            c0,c1,idx=struct.unpack_from('<HHI',d,o+8); e0,e1=c565(c0),c565(c1)
            pal=[e0,e1,(2*e0+e1)/3,(e0+2*e1)/3]
            for i in range(16): img[by*4+i//4,bx*4+i%4]=pal[(idx>>(2*i))&3]
            o+=16
    return img

def qm(q):
    x,y,z,w=q
    return np.array([[1-2*(y*y+z*z),2*(x*y-z*w),2*(x*z+y*w)],[2*(x*y+z*w),1-2*(x*x+z*z),2*(y*z-x*w)],[2*(x*z-y*w),2*(y*z+x*w),1-2*(x*x+y*y)]])

def skeleton(path):
    sk=[c for c in chunks(open(path,'rb').read()) if c[1]==0x29][0][2]
    n=sk[2]; pos=4; G=[]
    for i in range(n):
        p=sk[pos]; r=struct.unpack_from('<4f',sk,pos+2); t=np.array(struct.unpack_from('<3f',sk,pos+18)); pos+=30
        R=qm(r)
        G.append((R,t) if p==i else (G[p][0]@R, G[p][0]@t+G[p][1]))
    return G

def flip(p,ax):
    p=np.array(p,float)
    if ax in (1,2,3): p[ax-1]*=-1
    return p

def mesh_tris(me,G,tex):
    u=lambda o,f: struct.unpack_from(f,me,o)
    f3=me[2]; sym=me[4]==1; uja=bool(f3&0x80); cloth=bool(f3&1)
    ja=u(0xc,'<I')[0]*2; nj=u(0x10,'<H')[0]; vc=u(0x12,'<I')[0]*2; vm=u(0x18,'<I')[0]*2; vd=u(0x1e,'<I')[0]*2
    pal=[u(ja+2*i,'<H')[0] for i in range(nj)]
    J=lambda s: pal[s] if uja else s
    s,dc=u(vc,'<HH'); refs=[u(vm+4*i,'<HH') for i in range(s+dc)]
    W=[[],[]]; o=vd
    for i in range(s):
        p=u(o,'<3f'); o+=12 if cloth else 24; r=refs[i][0]
        for m in (0,1):
            if m and not sym: W[1].append(None); continue
            j=J(r&0x7f) if m==0 else J(r>>7&0x7f); pp=np.array(p) if m==0 else flip(p,r>>14)
            W[m].append(G[j][0]@pp+G[j][1])
    for i in range(dc):
        a=u(o,'<8f'); o+=32 if cloth else 56; r0,r1=refs[s+i]; p0=[a[0],a[2],a[4]]; p1=[a[1],a[3],a[5]]; w0,w1=a[6],a[7]
        for m in (0,1):
            if m and not sym: W[1].append(None); continue
            j0=J(r0&0x7f) if m==0 else J(r0>>7&0x7f); j1=J(r1&0x7f) if m==0 else J(r1>>7&0x7f)
            q0=np.array(p0) if m==0 else flip(p0,r0>>14); q1=np.array(p1) if m==0 else flip(p1,r1>>14)
            W[m].append(G[j0][0]@q0+w0*G[j0][1]+G[j1][0]@q1+w1*G[j1][1])
    c=u(0x6,'<I')[0]*2; tn=''; out=[]
    while True:
        op,=u(c,'<H'); c+=2
        if op==0xffff: break
        if op==0x8010: c+=0x2c; continue
        if op==0x8000: tn=me[c:c+16].decode('latin1'); c+=16; continue
        n,=u(c,'<H'); c+=2; T=[]
        if op==0x0054:
            for t in range(n): T.append((u(c,'<3H'),u(c+6,'<6f'))); c+=30
        elif op==0x5453:
            i3=u(c,'<3H'); uv=u(c+6,'<6f'); c+=30; idx=list(i3); uvs=[uv[0:2],uv[2:4],uv[4:6]]
            for t in range(n-1): idx.append(u(c,'<H')[0]); uvs.append(u(c+2,'<2f')); c+=10
            T=[(idx[k:k+3], sum(uvs[k:k+3],())) for k in range(n)]
        elif op==0x0043:
            for t in range(n): T.append((u(c,'<3H'),None)); c+=10
        elif op==0x4353:
            i3=u(c,'<3H'); c+=10; idx=list(i3)
            for t in range(n-1): idx.append(u(c,'<H')[0]); c+=2
            T=[(idx[k:k+3],None) for k in range(n)]
        for i3,uv in T:
            col=np.array([150,150,150])
            if uv and tn in tex:
                im=tex[tn]; h,w,_=im.shape; cu=(uv[0]+uv[2]+uv[4])/3%1; cv=(uv[1]+uv[3]+uv[5])/3%1
                col=im[min(int(cv*h),h-1),min(int(cu*w),w-1)]
            for m in (0,1):
                if W[m][i3[0]] is None: continue
                out.append(([W[m][x] for x in i3],col/255))
    return out

def load(skel, parts):
    G=skeleton(skel); tris=[]
    for path in parts:
        ch=chunks(open(path,'rb').read())
        tex={c[2][1:17].decode('latin1'): dxt3(c[2]) for c in ch if c[1]==0x20 and c[2][0]==0xa1 and c[2][0x39:0x3d]==b'3TXD'}
        for c in ch:
            if c[1]==0x2a: tris+=mesh_tris(c[2],G,tex)
    return tris

if __name__=='__main__':
    out=sys.argv[1]; skel=sys.argv[2]; parts=sys.argv[3:]
    tris=load(skel,parts)
    fig=plt.figure(figsize=(16,9),facecolor='#8fa3b8')
    for k,az in enumerate([-90,-30,90]):
        ax=fig.add_subplot(1,3,k+1,projection='3d'); ax.set_facecolor('#8fa3b8')
        polys=[np.array([[p[0],p[2],-p[1]] for p in t]) for t,_ in tris]
        ax.add_collection3d(Poly3DCollection(polys,facecolors=[c for _,c in tris],edgecolors='none'))
        allp=np.concatenate(polys); mn,mx=allp.min(0),allp.max(0); c_=(mn+mx)/2; r=(mx-mn).max()/2
        ax.set_xlim(c_[0]-r,c_[0]+r); ax.set_ylim(c_[1]-r,c_[1]+r); ax.set_zlim(c_[2]-r,c_[2]+r)
        ax.view_init(elev=8,azim=az); ax.set_axis_off()
    plt.tight_layout(); plt.savefig(out,dpi=80)
