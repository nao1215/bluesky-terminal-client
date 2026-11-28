"""Run the real-server suite using the release's official Compose setup."""

import json
import os
from pathlib import Path
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
RELEASES = {
    "misskey": (
        "2025.12.0",
        "https://raw.githubusercontent.com/misskey-dev/misskey/2025.12.0",
        "misskey/misskey:2025.12.0@sha256:17814737ef9ceeb63d11de8b913774d23eb7ae0a24570e6602f85a924d917714",
    ),
    "sharkey": (
        "2025.4.3",
        "https://activitypub.software/TransFem-org/Sharkey/-/raw/2025.4.3",
        "registry.activitypub.software/transfem-org/sharkey:2025.4.3@sha256:9d5e6ca3e324a644aa9f7475033bebd48b1f35941ad6d57bfb886282580ebe28",
    ),
}


def api(host, endpoint, body):
    request = urllib.request.Request(
        host + "/api/" + endpoint,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    # Do not follow redirects, including redirects from a local server to a public one.
    opener = urllib.request.build_opener(NoRedirect)
    for attempt in range(5):
        try:
            with opener.open(request, timeout=10) as response:
                data = response.read()
                return json.loads(data) if data else None
        except urllib.error.HTTPError as error:
            # A 429 explicitly refuses the request before execution. Do not
            # retry transport errors or 5xx responses to these setup writes.
            if error.code != 429 or attempt == 4:
                raise
            time.sleep(min(10, max(1, int(error.headers.get("Retry-After", "1")))))


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def probe(host, endpoint, body):
    try:
        return {"status": 200, "body": api(host, endpoint, body)}
    except urllib.error.HTTPError as error:
        return {"status": error.code, "body": json.loads(error.read())}


def capture(service, name, value):
    directory = os.environ.get("MSKY_CAPTURE_DIR")
    if directory:
        destination = Path(directory) / service
        destination.mkdir(parents=True, exist_ok=True)
        (destination / (name + ".json")).write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def main():
    service = sys.argv[1]
    version, upstream, image = RELEASES[service]
    for executable in ("docker", "cargo", "atago", "ffmpeg"):
        if not shutil.which(executable):
            raise RuntimeError(f"{executable} is required")
    subprocess.run(["cargo", "build", "--locked", "-p", "misskey-terminal-client"], cwd=ROOT, check=True)
    with tempfile.TemporaryDirectory(prefix=f"msky-{service}-") as folder:
        work = Path(folder)
        # The unprivileged application container must be able to read its configuration.
        work.chmod(0o755)
        config = work / ".config"
        config.mkdir()
        password = secrets.token_urlsafe(32)
        (config / "docker.env").write_text(
            f"POSTGRES_USER=msky\nPOSTGRES_PASSWORD={password}\nPOSTGRES_DB=msky\n"
        )
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            port = sock.getsockname()[1]
        host = f"http://127.0.0.1:{port}"
        # sqlLike is an officially supported provider on both releases.
        (config / "default.yml").write_text(
            f"url: {host}\nport: 3000\nid: aidx\n"
            f"db:\n  host: db\n  port: 5432\n  db: msky\n  user: msky\n  pass: {password}\n"
            "redis:\n  host: redis\n  port: 6379\n"
            "fulltextSearch:\n  provider: sqlLike\n"
        )
        official = work / "official.yml"
        with urllib.request.urlopen(upstream + "/compose_example.yml", timeout=30) as response:
            official.write_bytes(response.read())
        compose = json.loads(subprocess.check_output(
            ["docker", "compose", "-f", str(official), "config", "--format", "json"], cwd=work
        ))
        compose.pop("name", None)
        compose["networks"] = {"isolated": {"internal": True}, "ingress": {}}
        for name, digest in {
            "redis": "7f7d30b7fdd09467bac58198a1f166dd69789d348d4fe859cccb20e0cf287fbe",
            "db": ("0115267ab5a509022df9319c77be365cef335adb71556c75f60f5360ee247e71" if service == "misskey"
                   else "30b68193ce7e93707a406f81257b42dc34d635b42f3804cdaf927b8b749df1aa"),
        }.items():
            compose["services"][name]["image"] += "@sha256:" + digest
        compose["volumes"] = {name: {} for name in ("files", "db", "redis")}
        for name, definition in compose["services"].items():
            definition["networks"] = ["isolated"]
            definition["restart"] = "no"
            definition.pop("links", None)
            definition.pop("build", None)
            for volume in definition.get("volumes", []):
                if volume["target"].endswith("/.config"):
                    continue
                volume["type"] = "volume"
                volume["source"] = name if name != "web" else "files"
                volume.pop("bind", None)
                # PostgreSQL 18's official image stores versioned data below this directory.
                if name == "db" and definition["image"].startswith("postgres:18"):
                    volume["target"] = "/var/lib/postgresql"
        web = compose["services"]["web"]
        web["image"] = image
        web.pop("ports", None)
        web["healthcheck"] = {
            "test": ["CMD", "node", "-e", "fetch('http://127.0.0.1:3000/api/meta',"
                     "{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'})"
                     ".then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"],
            "interval": "2s", "timeout": "3s", "retries": 90, "start_period": "5s",
        }
        proxy = work / "nginx.conf"
        proxy.write_text(
            "events {}\nhttp { server { listen 3000; client_max_body_size 32m; "
            "location / { proxy_connect_timeout 1s; proxy_read_timeout 15s; proxy_pass http://web:3000; proxy_set_header Host $http_host; } } }\n"
        )
        # Only this fixed-upstream reverse proxy has ingress; the application,
        # database and Redis cannot initiate connections outside their network.
        compose["services"]["ingress"] = {
            "image": "nginx:1.28.0-alpine@sha256:30f1c0d78e0ad60901648be663a710bdadf19e4c10ac6782c235200619158284",
            "networks": ["isolated", "ingress"],
            "ports": [{"target": 3000, "published": str(port), "host_ip": "127.0.0.1"}],
            "volumes": [{"type": "bind", "source": str(proxy), "target": "/etc/nginx/nginx.conf", "read_only": True}],
            "depends_on": ["web"],
        }
        composed = work / "compose.json"
        composed.write_text(json.dumps(compose))
        project = "msky-compat-" + secrets.token_hex(6)
        command = ["docker", "compose", "-p", project, "-f", str(composed)]
        try:
            # Misskey's image does not contain a files directory; a new named
            # volume is root-owned. Match the image's application user.
            subprocess.run(command + ["pull"], check=True)
            user = subprocess.check_output(["docker", "image", "inspect", image, "--format", "{{.Config.User}}"], text=True).strip()
            if user:
                subprocess.run(command + ["run", "--rm", "--no-deps", "--user", "0", "--entrypoint", "sh", "web",
                                          "-c", f"chown {user} /{service}/files"], check=True)
            subprocess.run(command + ["up", "-d", "--wait", "--wait-timeout", "180"], check=True)
            deadline = time.monotonic() + 300
            while True:
                try:
                    meta = api(host, "meta", {"detail": True})
                    break
                except (urllib.error.URLError, TimeoutError, ConnectionError):
                    if time.monotonic() >= deadline:
                        raise RuntimeError("server did not answer /api/meta within 300 seconds") from None
                    time.sleep(2)
            assert meta["version"] == version, meta["version"]
            print(f"{service}: pinned {version}; /api/meta reports {meta['version']}; {host}", flush=True)
            admin = api(host, "admin/accounts/create", {
                "username": "compat_admin", "password": secrets.token_urlsafe(32),
            })
            second = api(host, "admin/accounts/create", {
                "i": admin["token"], "username": "compat_user", "password": secrets.token_urlsafe(32),
            })
            third = api(host, "admin/accounts/create", {
                "i": admin["token"], "username": "compat_pictures", "password": secrets.token_urlsafe(32),
            })
            permissions = ["read:account", "write:notes", "read:drive", "write:drive", "read:notifications",
                           "write:reactions", "read:chat", "write:chat", "read:following", "write:following", "write:notifications"]
            tokens = [api(host, "miauth/gen-token", {
                "i": account["token"], "session": None, "name": "msky local compatibility",
                "permission": permissions,
            })["token"] for account in (admin, second, third)]
            for name, endpoint, body in (
                ("search-unavailable", "notes/search", {"i": tokens[0], "query": "compat"}),
                ("chat", "chat/history", {"i": tokens[0], "limit": 10}),
                ("authentication", "i", {"i": secrets.token_urlsafe(32)}),
                ("missing-note", "notes/show", {"i": tokens[0], "noteId": "0000000000000000"}),
                ("role", "admin/emoji/add", {"i": second["token"], "name": "compat_denied", "fileId": "0000000000000000"}),
            ):
                capture(service, name, probe(host, endpoint, body))
            restricted = api(host, "miauth/gen-token", {"i": admin["token"], "session": None, "permission": []})["token"]
            capture(service, "permission", probe(host, "chat/history", {"i": restricted}))
            api(host, "admin/roles/update-default-policies", {
                "i": admin["token"], "policies": {"canSearchNotes": True},
            })
            from seed import seed
            seeded = seed(host, [admin, second, third], work, meta)
            env = os.environ.copy()
            env.update({
                "PATH": str(ROOT / "target/debug") + os.pathsep + env["PATH"],
                "MSKY_COMPOSE_PROJECT": project, "MSKY_COMPOSE_FILE": str(composed),
                "MSKY_ADMIN_TOKEN": admin["token"], "MSKY_RESTRICTED_TOKEN": restricted,
                "MSKY_HOST": host, "MSKY_TOKEN": tokens[0], "MSKY_SECOND_TOKEN": tokens[1], "MSKY_THIRD_TOKEN": tokens[2],
                "MSKY_CONFIG_DIR": str(work / "client-config"), "MSKY_RUN_MARKER": "mskycompat" + secrets.token_hex(8),
                "MSKY_COMPAT_DIR": str(HERE), "MSKY_COMPAT_SERVICE": service,
                "PYTHONDONTWRITEBYTECODE": "1", "LC_ALL": "C.UTF-8", "LANG": "C.UTF-8",
            })
            env.update({"MSKY_SEED_" + key.upper(): str(value) for key, value in seeded.items()
                        if isinstance(value, (str, int))})
            env["MSKY_SEED_NOTIFICATIONS"] = str(seeded["counts"]["notifications"])
            env["MSKY_SEED_DIR"] = str(work)
            env["MSKY_SKIP_GAP"] = "1"
            env["MSKY_STALE_RENOTE"] = "1" if seeded["stale_renote"] else ""
            env["MSKY_SERVER_SERVICE"] = service
            env.pop("LC_MESSAGES", None)
            report = subprocess.run(["atago", "run", "--report", "json", "--parallel", "1", *sys.argv[2:], str(HERE)],
                                    env=env, capture_output=True, text=True)
            print(report.stdout, flush=True)
            print(report.stderr, file=sys.stderr, flush=True)
            # Retain the report and public server version, never credentials or volumes.
            if os.environ.get("MSKY_REPORT_DIR"):
                destination = Path(os.environ["MSKY_REPORT_DIR"])
                destination.mkdir(parents=True, exist_ok=True)
                (destination / (service + ".json")).write_text(report.stdout)
                (destination / (service + "-server.json")).write_text(json.dumps({"version": meta["version"]}) + "\n")
            report.check_returncode()
        finally:
            # Remove named volumes as well as containers; all bind data belongs to TemporaryDirectory.
            subprocess.run(command + ["down", "--volumes", "--remove-orphans"], check=True)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, lambda *_: sys.exit(143))
    main()
