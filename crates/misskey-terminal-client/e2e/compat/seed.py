"""Seed the shared live catalog using only public APIs; no client assertions here."""

import json
from pathlib import Path
import subprocess
import struct
import zlib
import time
import urllib.request
import urllib.error

from server import api, capture, probe, NoRedirect


def upload(host, token, path, comment="compat media", force=True):
    boundary = "msky-local-seed-boundary"
    fields = {"i": token, "force": str(force).lower(), "comment": comment}
    parts = [f'--{boundary}\r\nContent-Disposition: form-data; name="{key}"\r\n\r\n{value}\r\n'.encode()
             for key, value in fields.items()]
    parts += [f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{path.name}"\r\nContent-Type: application/octet-stream\r\n\r\n'.encode(),
              path.read_bytes(), f'\r\n--{boundary}--\r\n'.encode()]
    request = urllib.request.Request(host + "/api/drive/files/create", data=b"".join(parts),
                                     headers={"Content-Type": f"multipart/form-data; boundary={boundary}"})
    with urllib.request.build_opener(NoRedirect).open(request, timeout=60) as response:
        return json.load(response)


def seed(host, accounts, work, meta):
    manifest = work / "seed.json"
    # The manifest belongs to this fresh volume set, and is written only after
    # every batch finishes. Reusing a completed seed within this run is a no-op.
    if manifest.exists():
        return json.loads(manifest.read_text())
    admin, video_user, picture_user = accounts
    token = admin["token"]
    api(host, "admin/roles/update-default-policies", {"i": token, "policies": {
        "canSearchNotes": True, "rateLimitFactor": 0, "maxFileSizeMb": 1,
    }})
    for account in accounts:
        for other in accounts:
            if account["id"] != other["id"]:
                api(host, "following/create", {"i": account["token"], "userId": other["id"]})
    media = work / "media"
    media.mkdir()
    video = media / "pattern.mp4"
    unsupported = media / "pattern-vp9.webm"
    for path, codec in ((video, "libx264"), (unsupported, "libvpx-vp9")):
        subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi",
                        "-i", "testsrc2=size=160x120:rate=8", "-t", "2", "-an", "-c:v", codec,
                        "-pix_fmt", "yuv420p", str(path)], check=True)
    picture = Path(__file__).resolve().parent.parent / "atago/testdata/photo.png"
    pics = [upload(host, picture_user["token"], picture)["id"] for _ in range(4)]
    film = upload(host, video_user["token"], video)["id"]
    badfilm = upload(host, token, unsupported)["id"]
    admin_pictures = [upload(host, token, picture)["id"] for _ in range(4)]
    admin_picture = admin_pictures[0]
    with urllib.request.build_opener(NoRedirect).open(host + "/nodeinfo/2.1", timeout=10) as response:
        nodeinfo = json.load(response)
    service = "sharkey" if "maxAltTextLength" in nodeinfo.get("metadata", {}) else "misskey"
    capture(service, "upload-dedup", {
        "reused": upload(host, token, picture, "compat replacement alt", force=False),
        "fresh": upload(host, token, picture, "compat replacement alt"),
    })
    api(host, "admin/emoji/add", {"i": token, "name": "compat", "fileId": upload(host, token, picture)["id"]})
    capture(service, "nodeinfo-limits",
            {"metadata": {key: value for key, value in nodeinfo.get("metadata", {}).items()
                          if key in ("maxAltTextLength", "maxNoteTextLength")}})
    data = {"video_path": str(video), "unsupported_path": str(unsupported), "picture_path": str(picture),
            "max_text": meta["maxNoteTextLength"], "max_alt": nodeinfo.get("metadata", {}).get("maxAltTextLength", 512),
            "max_file_bytes": 1048576, "counts": {}, "video_user": video_user["username"],
            "picture_user": picture_user["username"]}
    def note(account, text, **fields):
        try:
            return api(host, "notes/create", {"i": account["token"], "text": text, **fields})["createdNote"]["id"]
        except urllib.error.HTTPError as error:
            detail = json.loads(error.read()).get("error", {})
            raise RuntimeError(f"seeding {text[:40]}: {detail.get('code')}: {detail.get('message')}") from None
    texts = []
    for i in range(500):
        texts.append(note(admin, f"compat-text-{i:03} #compatcatalog"))
        api(host, "notes/reactions/create", {"i": video_user["token"], "noteId": texts[-1], "reaction": "👍"})
        if i % 100 == 99:
            print(f"seed: {i + 1} text notes and reaction notifications", flush=True)
    api(host, "notes/create", {"i": video_user["token"], "renoteId": texts[1]})
    for i in range(100):
        note(video_user, f"compat-video-{i:03}", fileIds=[film])
        note(picture_user, f"compat-picture-{i:03}", fileIds=pics)
    data["video_first"] = api(host, "users/notes", {"i": token, "userId": video_user["id"], "limit": 1})[0]["id"]
    data["picture_first"] = api(host, "users/notes", {"i": token, "userId": picture_user["id"], "limit": 1})[0]["id"]
    data["deep_root"] = note(admin, "compat-deep-root")
    leaf = data["deep_root"]
    for i in range(50):
        leaf = note(admin, f"compat-deep-{i:03}", replyId=leaf)
    data["deep_leaf"] = leaf
    data["wide_root"] = note(admin, "compat-wide-root")
    for i in range(200):
        note(admin, f"compat-wide-{i:03}", replyId=data["wide_root"])
    root = api(host, "notes/show", {"i": token, "noteId": data["wide_root"]})
    first = api(host, "notes/children", {"i": token, "noteId": data["wide_root"], "limit": 100})
    second = api(host, "notes/children", {"i": token, "noteId": data["wide_root"], "limit": 100, "untilId": first[-1]["id"]})
    # Keep the actual page boundaries and only the fields consumed by the
    # pagination regression. No user profiles or media URLs are needed.
    def page_fields(note):
        return {key: note[key] for key in ("id", "text", "replyId", "repliesCount")}
    capture(service, "thread-pages", {"root": page_fields(root),
            "first": list(map(page_fields, first)), "second": list(map(page_fields, second))})
    for count in range(1, 5):
        note(admin, f"compat-{count}-pictures", fileIds=admin_pictures[:count])
    for visibility in ("public", "home", "followers", "specified"):
        fields = {"visibility": visibility, "cw": "compat warning"}
        if visibility == "specified":
            fields["visibleUserIds"] = [video_user["id"]]
        data[visibility] = note(admin, f"compat-{visibility}", **fields)
    data["unicode"] = note(admin, "compat-unicode 👨‍👩‍👧 👍🏽 🇯🇵 ☀️ 漢字 العربية e\u0301 " + "w" * 180, fileIds=[admin_picture])
    api(host, "i/update", {"i": token, "name": "compat 👨‍👩‍👧 👍🏽 🇯🇵 漢字 العربية e\u0301"})
    data["unsupported"] = note(admin, "compat-unsupported-vp9", fileIds=[badfilm])
    data["max_text_note"] = note(admin, "x" * data["max_text"])
    data["mention"] = note(video_user, "@compat_admin compat-mention", fileIds=[film])
    data["reply"] = note(video_user, "compat-reply", fileIds=[film], replyId=texts[0])
    renote = api(host, "notes/create", {"i": video_user["token"], "renoteId": texts[0]})["createdNote"]["id"]
    api(host, "notes/delete", {"i": video_user["token"], "noteId": renote})
    data["renote_target"] = texts[0]
    data["stale_renote"] = api(host, "notes/show", {"i": token, "noteId": texts[0]})["renoteCount"] != 0
    data["blocked"] = note(picture_user, "compat-blocked", fileIds=pics, visibility="followers")
    api(host, "blocking/create", {"i": picture_user["token"], "userId": admin["id"]})
    capture(service, "blocked-note",
            probe(host, "notes/show", {"i": token, "noteId": data["blocked"]}))
    # Keep the full public media timeline available to the account used for
    # heavy scrolling; a fourth account is not needed for the blocked-note case.
    data["deleted"] = note(admin, "compat-deleted")
    api(host, "notes/delete", {"i": token, "noteId": data["deleted"]})
    data["counts"] = {"video_only": 102, "four_picture": 101, "deep_replies": 50,
                      "wide_replies": 200, "text": 500, "reaction_notifications": 500}
    # Notification workers are asynchronous. Read the public paginated API
    # before starting specs so the seeded count is an oracle, not a timing race.
    deadline = time.monotonic() + 60
    while True:
        notifications = []
        until = None
        while True:
            body = {"i": token, "limit": 100, "markAsRead": False}
            if until:
                body["untilId"] = until
            page = api(host, "i/notifications", body)
            notifications.extend(page)
            if not page:
                break
            until = page[-1]["id"]
        if sum(n["type"] == "reaction" for n in notifications) >= 500:
            break
        if time.monotonic() > deadline:
            raise RuntimeError(f"expected 500 reaction notifications, got {len(notifications)} notifications")
        time.sleep(1)
    data["counts"]["notifications"] = len(notifications)
    data["notification_types"] = sorted({n["type"] for n in notifications})
    data["max_text_value"] = "x" * data["max_text"]
    data["over_text_value"] = "x" * (data["max_text"] + 1)
    data["max_alt_text"] = "a" * data["max_alt"]
    data["over_alt_text"] = "a" * (data["max_alt"] + 1)
    # A valid PNG with padding tests the server's byte limit without a huge fixture.
    for name, size in (("max-file.png", 1048576), ("over-file.png", 1048577)):
        original = picture.read_bytes()
        payload = b"raNd" + bytes(size - len(original) - 12)
        chunk = struct.pack(">I", len(payload) - 4) + payload + struct.pack(">I", zlib.crc32(payload))
        (work / name).write_bytes(original[:-12] + chunk + original[-12:])
    manifest.write_text(json.dumps(data, indent=2) + "\n")
    print("seed counts: " + json.dumps(data["counts"], sort_keys=True), flush=True)
    return data
