"""Public NIP-01 seed data; three generated accounts and locally hosted media."""

import json


def events(protocol, keys, now, media):
    owner, pictures, videos = keys
    public = [protocol.public_of(key) for key in keys]
    base = now - 2000
    output = []
    counts = {"text": 100, "video": 100, "four_pictures": 800, "deep_replies": 50, "wide_replies": 200, "mentions": 802}
    alt = "CJK 日本語 👨‍👩‍👧 👍🏽 🇯🇵 ✈️ עברית é " + "longword" * 15

    def make(key, timestamp, kind, tags, content):
        event = protocol.make(key, timestamp, kind, tags, content)
        output.append(event)
        return event

    def images():
        return [["imeta", f"url {media}/picture-{i}.png", "m image/png", f"alt {alt}"] for i in range(4)]

    for i, key in enumerate(keys):
        make(key, base - 10, 0, [], json.dumps({"name": ["compat-user", "compat-pictures", "compat-videos"][i], "about": "local relay compatibility " + alt}))
        make(key, base - 9, 3, [["p", p] for p in public if p != public[i]], "")
    for i in range(100):
        tags = images()[:i + 1] + [["t", f"media-{i + 1}"]] if i < 3 else []
        make(owner, base + i, 1, tags + [["t", "catalog"], ["t", "text"]], f"text {i:03} {alt}")
        make(pictures, base + 100 + i, 1, images() + [["p", public[0]], ["t", "catalog"], ["t", "pictures"]], f"pictures {i:03}")
        codec = "unsupported.webm" if i == 0 else "pattern.mp4"
        make(videos, base + 200 + i, 1, [["t", "catalog"], ["t", "video"], ["imeta", f"url {media}/{codec}", "m video/webm" if i == 0 else "m video/mp4"]], f"video {i:03} {media}/{codec}")
    for i in range(500):
        make(pictures, base + 300 + i, 1, images() + [["p", public[0]], ["t", "catalog"], ["t", "pictures"]], f"mention {i:03}")
    root = make(owner, base + 800, 1, [["t", "catalog"], ["t", "deep"]], "depth 000")
    parent = root
    for i in range(1, 51):
        parent = make(owner, base + 800 + i, 1, [["t", "catalog"], ["t", "deep"], ["e", root["id"], "", "root"], ["e", parent["id"], "", "reply"]], f"depth {i:03}")
    root = make(owner, base + 851, 1, [["t", "catalog"], ["t", "wide"]], "wide root")
    for i in range(200):
        make(pictures, base + 852 + i, 1, images() + [["t", "catalog"], ["t", "pictures"], ["t", "wide"], ["p", public[0]], ["e", root["id"], "", "reply"]], f"wide reply {i:03}")
    parent = make(owner, now - 60, 1, [["t", "catalog"], ["t", "cw"], ["content-warning", "spoilers"]], "compat own note")
    make(pictures, now - 40, 1, images() + [["t", "catalog"]], "compat followed note")
    make(pictures, now - 30, 7, [["p", public[0]], ["e", parent["id"]]], "+")
    make(videos, now - 29, 6, [["p", public[0]], ["e", parent["id"]]], json.dumps(parent, separators=(",", ":")))
    make(videos, now - 20, 1, [["t", "catalog"], ["p", public[0]], ["imeta", f"url {media}/pattern.mp4", "m video/mp4"]], "compat mention")
    make(pictures, now - 10, 1, images() + [["t", "catalog"], ["p", public[0]], ["e", parent["id"], "", "reply"]], "compat reply")
    counts["four_pictures"] = sum(sum(t[0] == "imeta" and "m image/png" in t for t in e["tags"]) == 4 for e in output)
    counts["video"] = sum(any(t[0] == "imeta" and any(v.startswith("m video/") for v in t) for t in e["tags"]) for e in output)
    counts["kind1"] = sum(e["kind"] == 1 for e in output)
    counts["notifications"] = sum(e["kind"] in [1, 3, 6, 7] and ["p", public[0]] in e["tags"] for e in output)
    counts["events"] = len(output)
    return output, counts
