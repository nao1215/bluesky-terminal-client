"""Assertions for public writes; expected values are independent of Rust builders."""
import json
import sys
from pathlib import Path
from relay import ALICE, BOB, public_of, digest, verify
kind = int(sys.argv[1])
e = json.loads(Path('published.json').read_text())
assert e['kind'] == kind, e
assert e['pubkey'] == public_of(ALICE), e
assert digest(e).hex() == e['id']
assert verify(bytes.fromhex(e['pubkey']), bytes.fromhex(e['id']), bytes.fromhex(e['sig']))
seed = json.loads(Path('seed.json').read_text())
note = next(n for n in seed if n['kind'] == 1 and n['pubkey'] == public_of(ALICE))
if kind == 7:
    assert e['tags'] == [['e', note['id']], ['p', note['pubkey']], ['k', '1']]
    assert e['content'] == '+'
elif kind == 6:
    assert e['tags'] == [['e', note['id'], Path('ready.txt').read_text()], ['p', note['pubkey']]]
    assert json.loads(e['content']) == note
elif kind == 5:
    assert any(t[0] == 'e' for t in e['tags'])
    assert any(t[0] == 'k' for t in e['tags'])
elif kind == 10000:
    assert e['tags'] == [['p', public_of(BOB)]]
elif kind == 3:
    assert ['p', 'not-a-key'] in e['tags'], 'unrelated contact tag lost'
elif kind == 1984:
    assert e['tags'] == [['p', note['pubkey'], 'spam'], ['e', note['id'], 'spam']]
    assert e['content'] == 'fixture report'
elif kind == 0:
    assert json.loads(e['content'])['picture'] == 'http://127.0.0.1:9/alice.png'
    assert json.loads(e['content'])['name'] == 'updated'
elif kind == 10050:
    assert e['tags'] == [['relay', Path('ready.txt').read_text()]]
elif kind == 1:
    assert ['content-warning', 'spoiler'] in e['tags']
    assert ['q', note['id'], Path('ready.txt').read_text(), note['pubkey']] in e['tags']
print('exact event verified')
