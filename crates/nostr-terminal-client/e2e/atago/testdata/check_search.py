"""Assert local relay traffic for the search box; never open a connection."""
import json
import sys
from pathlib import Path

messages = [json.loads(line) for line in Path("requests.jsonl").read_text().splitlines()]
assert not any(m[0] == "EVENT" for m in messages), messages
filters = [m[2] for m in messages if m[0] == "REQ"]
mode = sys.argv[1]
if mode == "restore":
    home = [f for f in filters if f.get("authors") and f.get("kinds") == [1]]
    # One UI load may make several protocol requests to exhaust a short page.
    assert len([f for f in home if "until" not in f]) == 1, home
    first_search = next(i for i, f in enumerate(filters) if f.get("search"))
    assert not any(f in home for f in filters[first_search:]), filters
    assert len([f for f in filters if f.get("search") == "carol" and "until" not in f]) == 2, filters
    assert any(f.get("#t") == ["absent"] for f in filters), filters
    assert not any(f.get("search") == "discard this" for f in filters), filters
elif mode == "pages":
    search = [f for f in filters if f.get("search") == "boundary"]
    assert any("until" not in f and f["limit"] == 20 for f in search), search
    assert any(f.get("until") == 1767226600 and f["limit"] >= 40 for f in search), search
    columns = json.loads(next(Path("cfg").glob("state-*.json")).read_text())["columns"]
    assert "search:boundary" in columns, columns
else:
    raise AssertionError(mode)
print("checked " + mode)
