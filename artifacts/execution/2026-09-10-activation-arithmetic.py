"""Operator-only Python-port/Fraction check; this does not execute Rust."""
import random, struct, fractions, json, hashlib
from pathlib import Path
F = fractions.Fraction
MASK = (1 << 64) - 1
r = random.Random(731901)
def f32(u): return struct.unpack('>f', struct.pack('>I', u))[0]
def finite():
    while True:
        u = r.getrandbits(32)
        if (u >> 23) & 255 != 255: return u
def part(u):
    e = (u >> 23) & 255
    return ((u & 0x7fffff) | (0x800000 if e else 0)), max(0,e-1)
def add_word(a,i,v):
    while v:
        assert i < 9
        z = a[i] + v; a[i] = z & MASK; v = z >> 64; i += 1
def add_product(pos,neg,a,b):
    ma,ea = part(a); mb,eb = part(b); v=ma*mb; sh=ea+eb; i=sh//64; off=sh%64
    words = neg if (a ^ b) >> 31 else pos
    add_word(words,i,(v << off) & MASK)
    if off: add_word(words,i+1,v >> (64-off))
def finish(pos,neg):
    a = sum(x << (64*i) for i,x in enumerate(pos)); b = sum(x << (64*i) for i,x in enumerate(neg))
    big,small=(neg,pos) if a<b else (pos,neg); out=[]; borrow=0
    for x,y in zip(big,small):
        first=x-y; next_=(first & MASK)-borrow; out.append(next_ & MASK); borrow=int(first<0 or next_<0)
    assert not borrow
    mag=sum(x << (64*i) for i,x in enumerate(out))
    assert mag == abs(a-b)
    return -mag if a<b else mag
def score(xs,ws,bias=0,threshold=0):
    pos=[0]*9; neg=[0]*9
    for a,b in zip(xs,ws): add_product(pos,neg,a,b)
    add_product(pos,neg,bias,0x3f800000); add_product(pos,neg,threshold ^ 0x80000000,0x3f800000)
    return finish(pos,neg)
def oracle(xs,ws,bias,threshold):
    q=sum((F(f32(a))*F(f32(b)) for a,b in zip(xs,ws)), F(f32(bias)))-F(f32(threshold))
    q *= 1 << 298
    assert q.denominator == 1
    return q.numerator
cases=384; interval_cases=0
for n in range(cases):
    dim=1+n%31; xs=[finite() for _ in range(dim)]; ws=[finite() for _ in range(dim)]; bias=finite(); threshold=finite()
    exact=score(xs,ws,bias,threshold)
    assert exact == oracle(xs,ws,bias,threshold)
    prev=None
    for bits in range(24):
        mask=(1 << (23-bits))-1
        los=[]; his=[]
        for x,w in zip(xs,ws):
            low=x & ~mask; high=low|mask
            if (x ^ w) >> 31: low,high=high,low
            los.append(low); his.append(high)
        lo=score(los,ws,bias,threshold); hi=score(his,ws,bias,threshold)
        assert lo<=exact<=hi
        if prev: assert prev[0]<=lo and hi<=prev[1]
        prev=(lo,hi); interval_cases+=1
    assert lo==exact==hi
assert score([0x7f7fffff,1,0x7f7fffff],[0x7f7fffff,1,0xff7fffff])==1
max_sum=score([0x7f7fffff]*65536,[0x7f7fffff]*65536,0x7f7fffff,0xff7fffff)
assert max_sum.bit_length()<=576
out={'scope':'Python port of proposed arithmetic against Fraction oracle; NOT Rust execution',
     'script_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
     'seed':731901,'finite_dot_products':cases,'nested_interval_checks':interval_cases,
     'extreme_dimensions':65536,'maximum_sum_bits':max_sum.bit_length(),
     'lost_residual_exact_units':1,'mismatches':0}
print(json.dumps(out,indent=2))
