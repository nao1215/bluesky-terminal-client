"""Check the exact writes/filters recorded by the local relay; no network."""
import json
import sys
from pathlib import Path

messages = [json.loads(line) for line in Path("requests.jsonl").read_text(encoding="utf-8").splitlines()]
writes = [message[1] for message in messages if message[0] == "EVENT"]
seed = json.loads(Path("seed.json").read_text(encoding="utf-8"))
parent = next(event for event in seed if event["content"] == "返信です")
root = next(event for event in seed if event["content"] == "はじめての投稿 🐈‍⬛")
alice = root["pubkey"]
repost_content = json.dumps({k: parent[k] for k in ("id", "pubkey", "created_at", "kind", "tags", "content", "sig")}, ensure_ascii=False, separators=(",", ":"))
mode = sys.argv[1]
if mode == "none":
    assert writes == [], writes
elif mode == "profile-chat":
    assert sorted(e['kind'] for e in writes) == [0, 1059, 1059], writes
    profile = next(e for e in writes if e['kind'] == 0)
    assert json.loads(profile['content'])['name'] == 'TUI profile'
    assert json.loads(profile['content'])['picture'] == 'http://127.0.0.1:9/alice.png'
    assert profile['tags'] == []
elif mode == "writes":
    assert len(writes) == 4, writes
    new = next(e for e in writes if e["kind"] == 1 and e["content"] == "new note\nsecond line")
    assert new["tags"] == [], new
    reply = next(e for e in writes if e["kind"] == 1 and e["content"] == "reply from tui")
    assert reply["tags"] == [["e", root["id"], "", "root"], ["e", parent["id"], "", "reply"]] + [["p", p] for p in sorted({alice, parent["pubkey"]})], reply
    reaction = next(e for e in writes if e["kind"] == 7)
    assert reaction["content"] == "+" and reaction["tags"] == [["e", parent["id"]], ["p", parent["pubkey"]], ["k", "1"]], reaction
    repost = next(e for e in writes if e["kind"] == 6)
    assert repost["tags"] == [["e", parent["id"], Path("ready.txt").read_text()], ["p", parent["pubkey"]]], repost
    assert repost["content"] == repost_content, repost
    assert all(e["pubkey"] == alice for e in writes), writes
elif mode == "reject":
    assert sorted(e["kind"] for e in writes) == [1, 6, 7], writes
    assert next(e for e in writes if e["kind"] == 1)["content"] == "keep this draft"
    for event in writes:
        if event["kind"] == 1:
            assert event["tags"] == []
        else:
            tags = [["e", parent["id"]], ["p", parent["pubkey"]]]
            if event["kind"] == 7:
                tags.append(["k", "1"])
                assert event["content"] == "+"
            else:
                tags[0].append(Path("ready.txt").read_text())
                assert event["content"] == repost_content
            assert event["tags"] == tags
elif mode == "pages":
    assert writes == []
    filters = [m[2] for m in messages if m[0] == "REQ" and 1 in m[2]["kinds"]]
    assert any(f.get("until") == 1767226600 and f["limit"] >= 40 for f in filters), filters
    assert any(parent["id"] in f.get("#e", []) for f in filters), filters
    assert any(f.get("ids") == [root["id"]] for f in filters), filters
else:
    raise AssertionError(mode)
print("checked " + mode)
