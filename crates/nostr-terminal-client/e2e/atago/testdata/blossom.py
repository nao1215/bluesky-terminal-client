"""Local-only BUD-02 server; verifies exact authorization, digest and stripped bytes."""
import base64
import hashlib
import http.server
import json
import time
from pathlib import Path
import relay

class Blossom(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_PUT(self):
        assert self.path == '/upload'
        auth = self.headers['Authorization']
        assert auth.startswith('Nostr ')
        event = json.loads(base64.b64decode(auth[6:]))
        body = self.rfile.read(int(self.headers['Content-Length']))
        digest = hashlib.sha256(body).hexdigest()
        assert event['kind'] == 24242
        assert relay.digest(event).hex() == event['id']
        assert relay.verify(bytes.fromhex(event['pubkey']), bytes.fromhex(event['id']), bytes.fromhex(event['sig']))
        assert ['t', 'upload'] in event['tags']
        assert ['x', digest] in event['tags']
        expiry = int(next(t[1] for t in event['tags'] if t[0] == 'expiration'))
        assert time.time() < expiry <= time.time() + 301
        assert b'SecretCam' not in body and b'private comment' not in body
        Path('upload.bin').write_bytes(body)
        Path('upload-auth.json').write_text(json.dumps(event))
        extension = 'mp4' if self.headers['Content-Type'] == 'video/mp4' else 'png'
        if not hasattr(self.server, 'blobs'):
            self.server.blobs = {}
        self.server.blobs[f'/{digest}.{extension}'] = (body, self.headers['Content-Type'])
        response = json.dumps({'sha256': digest, 'size': len(body), 'type': self.headers['Content-Type'], 'url': f'http://127.0.0.1:{self.server.server_port}/{digest}.{extension}'}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    def do_GET(self):
        blob = getattr(self.server, 'blobs', {}).get(self.path)
        if blob is None:
            self.send_error(404)
            return
        body, mime = blob
        self.send_response(200)
        self.send_header('Content-Type', mime)
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)
