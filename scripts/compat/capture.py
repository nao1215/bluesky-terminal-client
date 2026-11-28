#!/usr/bin/env python3
"""Capture public API answers from this run's throwaway users, never OAuth."""
import json
from pathlib import Path
import sys

from server import API


def capture(work, manifest):
    users = json.loads((work / 'users.json').read_text())
    api = API(manifest['host'], users[0]['token'])
    public = API(manifest['host'])
    responses = {'system': manifest['system'], 'verified_at': manifest['verified_at']}

    def record(name, client, path, **kwargs):
        body, _ = client.request(path, allow_error=True, **kwargs)
        try:
            body = json.loads(body)
        except ValueError:
            body = body.decode()
        responses[name] = {**client.response, 'body': body}
        return body

    record('instance_v1', public, '/api/v1/instance')
    record('instance_v2', public, '/api/v2/instance')
    discovery = record('discovery', public, '/.well-known/nodeinfo')
    for link in discovery.get('links', []):
        if link.get('rel', '').endswith(('/2.0', '/2.1')):
            record('nodeinfo', public, link['href'])
            break
    record('status', api, '/api/v1/statuses/' + manifest['ids']['unicode'])
    record('video', api, '/api/v1/statuses/' + manifest['ids']['viewer_video'])
    record('page', api, '/api/v1/accounts/' + manifest['accounts'][2]['id'] + '/statuses?limit=2')
    record('conversations', api, '/api/v1/conversations')
    if manifest.get('multipart_limit'):
        record('chats', api, '/api/v1/pleroma/chats')
    record('deleted', api, '/api/v1/statuses/' + manifest['ids']['deleted'])
    record('wrong_token', API(manifest['host'], 'invalid-compat-token'), '/api/v1/accounts/verify_credentials')
    second = API(manifest['host'], users[1]['token'])
    account = '/api/v1/accounts/' + manifest['accounts'][0]['id']
    second.json(account + '/block', {})
    try:
        record('blocked', api, '/api/v1/statuses/' + manifest['ids']['reply'])
    finally:
        second.json(account + '/unblock', {})
        second.json(account + '/follow', {})
        api.json('/api/v1/accounts/' + manifest['accounts'][1]['id'] + '/follow', {})
    for key in ('markdown', 'mfm', 'extension_quote', 'emoji_reaction'):
        if key in manifest['ids']:
            record(key, api, '/api/v1/statuses/' + manifest['ids'][key])
    if manifest.get('multipart_limit'):
        record('text_over_limit', api, '/api/v1/statuses', data={'status': 'x' * (manifest['limits']['statuses']['max_characters'] + 1)})
        oversized = work / 'over-server.mp4'
        try:
            with oversized.open('wb') as out:
                out.write((work / 'clip.mp4').read_bytes())
                out.truncate(manifest['limits']['media_attachments']['video_size_limit'])
            record('upload_over_limit', api, '/api/v2/media', files=[('file', oversized, 'video/mp4')])
        finally:
            oversized.unlink(missing_ok=True)
    (work / 'responses.json').write_text(json.dumps(responses, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    work = Path(sys.argv[1])
    capture(work, json.loads((work / 'seed.json').read_text()))
