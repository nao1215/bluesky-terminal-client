"""Independent stdlib-only NIP-44/NIP-59 decoding of the binary's wire events."""
import base64
import hashlib
import hmac
import json
import struct
from pathlib import Path
from relay import ALICE, BOB, mul, lift_x, public_of, digest, verify

def crypt(key, nonce, data):
    def rotate(x,n): return ((x << n) | (x >> (32-n))) & 0xffffffff
    def quarter(s,a,b,c,d):
        s[a]=(s[a]+s[b])&0xffffffff; s[d]=rotate(s[d]^s[a],16)
        s[c]=(s[c]+s[d])&0xffffffff; s[b]=rotate(s[b]^s[c],12)
        s[a]=(s[a]+s[b])&0xffffffff; s[d]=rotate(s[d]^s[a],8)
        s[c]=(s[c]+s[d])&0xffffffff; s[b]=rotate(s[b]^s[c],7)
    out=bytearray()
    for offset in range(0,len(data),64):
        state=list(struct.unpack('<4I',b'expand 32-byte k'))+list(struct.unpack('<8I',key))+[offset//64]+list(struct.unpack('<3I',nonce))
        work=state[:]
        for _ in range(10):
            for indices in [(0,4,8,12),(1,5,9,13),(2,6,10,14),(3,7,11,15),(0,5,10,15),(1,6,11,12),(2,7,8,13),(3,4,9,14)]:quarter(work,*indices)
        stream=struct.pack('<16I',*[(a+b)&0xffffffff for a,b in zip(state,work)])
        out.extend(a^b for a,b in zip(data[offset:offset+64],stream))
    return bytes(out)

def decrypt(secret,public,payload):
    shared=mul(lift_x(int(public,16)),secret)[0].to_bytes(32,'big')
    conversation=hmac.digest(b'nip44-v2',shared,'sha256')
    raw=base64.b64decode(payload); assert raw[0]==2
    nonce=raw[1:33]; previous=b''; expanded=b''
    for i in range(1,4):
        previous=hmac.digest(conversation,previous+nonce+bytes([i]),'sha256');expanded+=previous
    assert hmac.compare_digest(raw[-32:],hmac.digest(expanded[44:76],raw[1:-32],'sha256'))
    padded=crypt(expanded[:32],expanded[32:44],raw[33:-32]);length=int.from_bytes(padded[:2],'big')
    assert all(b==0 for b in padded[2+length:])
    return json.loads(padded[2:2+length])

def valid(e):
    assert digest(e).hex()==e['id']
    assert verify(bytes.fromhex(e['pubkey']),bytes.fromhex(e['id']),bytes.fromhex(e['sig']))

if __name__=='__main__':
    writes=[json.loads(s)[1] for s in Path('requests.jsonl').read_text().splitlines() if json.loads(s)[0]=='EVENT']
    wraps=[e for e in writes if e['kind']==1059]
    assert len(wraps)==2, wraps
    assert len({e['pubkey'] for e in wraps})==2
    rumors=[]
    for secret in (ALICE,BOB):
        w=next(e for e in wraps if e['tags']==[['p',public_of(secret)]])
        valid(w); assert w['pubkey'] not in [public_of(ALICE),public_of(BOB)]
        seal=decrypt(secret,w['pubkey'],w['content']);valid(seal);assert seal['kind']==13 and seal['tags']==[]
        rumor=decrypt(secret,seal['pubkey'],seal['content']);assert 'sig' not in rumor
        assert rumor['kind']==14 and rumor['pubkey']==seal['pubkey']==public_of(ALICE)
        assert rumor['tags']==[['p',public_of(BOB)]]
        assert digest(rumor).hex()==rumor['id'];assert rumor['content']=='secret fixture message'
        rumors.append(rumor)
    assert rumors[0]==rumors[1]
    print('gift wraps verified independently')
