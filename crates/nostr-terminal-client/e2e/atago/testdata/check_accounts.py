"""Two public identities must keep separate read state; only fixture files are used."""
import json
import os
import subprocess
from pathlib import Path
from relay import ALICE, BOB, public_of

env = dict(os.environ, NSTR_CONFIG_DIR=str(Path('cfg').resolve()))
def run(*args):
    p = subprocess.run(['nstr', *args, '--json'], env=env, text=True, capture_output=True)
    assert p.returncode == 0, p.stderr
    return [json.loads(line) for line in p.stdout.splitlines()]

run('login', 'bob', '--public-key', public_of(BOB), '--relay', Path('ready.txt').read_text())
a = run('--account', 'default', 'notifications', '--seen')
b = run('--account', 'bob', 'notifications')
assert a and all(row['isRead'] for row in a)
assert not any(row['isRead'] for row in b)
run('--account', 'bob', 'notifications', '--seen')
aset = set(json.loads(Path(f'cfg/state-{public_of(ALICE)}.json').read_text())['seen'])
bset = set(json.loads(Path(f'cfg/state-{public_of(BOB)}.json').read_text())['seen'])
assert aset == {row['id'] for row in a}
assert bset == {row['id'] for row in b}
assert aset.isdisjoint(bset)
print('account read state isolated')
