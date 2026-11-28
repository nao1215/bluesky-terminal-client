"""Exercise downloads and dry-run/confirmation boundaries with fixture-owned files."""
import json
import os
import subprocess
from pathlib import Path
import sys
mode=sys.argv[1]
if mode=='setup':
    # A PNG containing a tEXt private comment; removing it leaves the pixels intact.
    import struct,zlib
    def chunk(k,v):return struct.pack('>I',len(v))+k+v+struct.pack('>I',zlib.crc32(k+v))
    Path('private.png').write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,2,0,0,0))+chunk(b'tEXt',b'Comment\0private comment SecretCam')+chunk(b'IDAT',zlib.compress(b'\0\xff\0\0'))+chunk(b'IEND',b''))
elif mode=='tui':
    writes=[json.loads(s)[1] for s in Path('requests.jsonl').read_text(encoding="utf-8").splitlines() if json.loads(s)[0]=='EVENT']
    assert sorted(e['kind'] for e in writes) == [0,1,1,1], writes
    posted=next(e for e in writes if e['content'].startswith('browser attachment'))
    assert len(posted['tags'])==1 and posted['tags'][0][0]=='imeta'
    assert 'alt browser alt' in posted['tags'][0]
    assert posted['content'].splitlines()[1] == posted['tags'][0][1][4:]
    profile=next(e for e in writes if e['kind']==0)
    assert len(profile['tags'])==1 and profile['tags'][0][0]=='imeta'
    assert json.loads(profile['content'])['picture']==profile['tags'][0][1][4:]
    assert json.loads(profile['content'])['display_name']=='Alice 🐈‍⬛'
    assert b'private comment' not in Path('upload.bin').read_bytes()
    print('browser post and profile uploads verified')
else:
    event=json.loads(Path('published.json').read_text(encoding="utf-8"))
    imeta=next(t for t in event['tags'] if t[0]=='imeta')
    assert 'alt fixture alt' in imeta and 'm image/png' in imeta
    url=next(t[4:] for t in imeta if t.startswith('url '));assert url in event['content']
    env=dict(os.environ,NSTR_CONFIG_DIR=str(Path('cfg').resolve()))
    def run(*args,stdin=None):
        p=subprocess.run(['nstr',*args,'--json'],env=env,input=stdin,text=True, encoding="utf-8",capture_output=True)
        assert p.returncode==0,(args,p.stderr);return p.stdout
    note=json.loads(run('note',event['id']));assert note['files'][0]['description']=='fixture alt'
    saved=json.loads(run('download',event['id'],'--output','saved'));assert Path(saved['path']).read_bytes()==Path('upload.bin').read_bytes()
    before=Path('requests.jsonl').read_bytes()
    run('delete',event['id'],stdin='n\n');run('report',event['id'],stdin='n\n')
    assert Path('requests.jsonl').read_bytes()==before,'declining contacted relay'
    upload_before=Path('upload-auth.json').read_bytes()
    value=json.loads(run('post','-','--image','private.png','--dry-run',stdin='stdin draft'))
    assert value['dryRun'] is True and value['text']=='stdin draft'
    assert Path('requests.jsonl').read_bytes()==before and Path('upload-auth.json').read_bytes()==upload_before
    print('uploads downloads and no-send boundaries verified')
