#!/usr/bin/env python3
"""Seed through public APIs, then run native atago specs against those IDs.

Only seeding lives here: assertions, process timing and PTY interactions belong
in atago. A private manifest makes seeding idempotent within one disposable run.
"""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.parse

from server import API
from capture import capture


def upload(api, path, alt):
    mime = 'image/png' if path.suffix == '.png' else ('video/webm' if path.suffix == '.webm' else 'video/mp4')
    media = api.json('/api/v2/media', {'description': alt}, files=[('file', path, mime)])
    for _ in range(120):
        if media.get('url'):
            return media['id']
        time.sleep(0.5)
        media = api.json('/api/v1/media/' + media['id'])
    raise RuntimeError('seed media processing exceeded 60 seconds')


def sized_videos(work, size, multipart_limit=False):
    # A standard MP4 free box pads an otherwise playable H.264 file exactly.
    clip = (work / 'clip.mp4').read_bytes()
    for name, total in [('max-video.mp4', size - 1), ('over-video.mp4', size)]:
        if multipart_limit:
            # mstdn uses "mstdn-" plus 32 hex digits, no description here.
            boundary = 'mstdn-' + '0' * 32
            overhead = len(f'--{boundary}\r\nContent-Disposition: form-data; name="file"; filename="{name}"\r\nContent-Type: video/mp4\r\n\r\n\r\n--{boundary}--\r\n'.encode())
            total = size - overhead + (name == 'over-video.mp4')
        padding = total - len(clip)
        if not 8 <= padding < 2**32:
            raise RuntimeError('the published video size cannot hold the test clip')
        with (work / name).open('wb') as out:
            out.write(clip)
            out.write(padding.to_bytes(4, 'big') + b'free')
            out.truncate(total)


def seed(work, system):
    heavy = os.environ.get('COMPAT_GROUP', 'heavy') == 'heavy'
    wide_count, deep_count, mention_count, picture_count = (200, 50, 500, 100) if heavy else (1, 1, 100, 1)
    host = os.environ['MSTDN_HOST']
    users = json.loads((work / 'users.json').read_text())
    apis = [API(host, u['token']) for u in users]
    accounts = [api.json('/api/v1/accounts/verify_credentials') for api in apis]
    for api in apis:
        api.json('/api/v1/accounts/update_credentials', {'locked': False}, method='PATCH')
    for index, api in enumerate(apis):
        for other, account in enumerate(accounts):
            if index != other:
                api.json('/api/v1/accounts/' + account['id'] + '/follow', {})
    main, second, video = apis
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'testsrc2=size=160x96:rate=12', '-t', '1', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-an', str(work / 'clip.mp4')], check=True)
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'testsrc2=size=160x96:rate=12', '-t', '1', '-c:v', 'libvpx-vp9', '-an', str(work / 'unsupported.webm')], check=True)
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'lavfi', '-i', 'color=c=blue:s=64x64', '-frames:v', '1', '-threads', '1', str(work / 'picture.png')], check=True)
    manifest = {'system': system, 'host': host, 'accounts': accounts, 'counts': {}, 'ids': {}, 'limits': main.json('/api/v1/instance').get('configuration', {})}
    try:
        manifest['instance'] = main.json('/api/v2/instance')
    except RuntimeError:
        manifest['instance'] = main.json('/api/v1/instance')
    instance = manifest['instance']
    metadata = instance.get('pleroma', {}).get('metadata', {})
    limits = instance.get('configuration', {})
    media = limits.get('media_attachments', {})
    manifest['limits'] = {
        'statuses': {'max_characters': limits.get('statuses', {}).get('max_characters', instance.get('max_toot_chars', 500))},
        'media_attachments': {
            'description_limit': media.get('description_limit', instance.get('description_limit', metadata.get('description_limit', 1500))),
            'video_size_limit': media.get('video_size_limit', instance.get('upload_limit')),
        },
    }
    manifest['multipart_limit'] = 'pleroma_api' in metadata.get('features', [])
    manifest['verified_at'] = datetime.now(timezone.utc).isoformat()
    discovery = main.json('/.well-known/nodeinfo')
    for link in discovery.get('links', []):
        if link.get('rel', '').endswith(('/2.0', '/2.1')):
            manifest['nodeinfo'] = API(host).json(link['href'])
            break
    ids, counts = manifest['ids'], manifest['counts']
    # Fail on a broken media deployment before creating hundreds of posts.
    first_video = upload(video, work / 'clip.mp4', 'Video 000')

    def post(api, text, **fields):
        made = api.json('/api/v1/statuses', {'status': text, 'visibility': 'public', **fields})['id']
        counts['posts_created'] = counts.get('posts_created', 0) + 1
        return made

    ids['text'] = post(main, 'compatibility root #compatroot')
    ids['reply'] = post(second, '@alice compatibility reply', in_reply_to_id=ids['text'])
    manifest['quote_posts'] = main.json('/api/v1/statuses/' + ids['text']).get('quote_approval') is not None or 'quote_posting' in metadata.get('features', [])
    if manifest['multipart_limit']:
        for key, content_type in [('markdown', 'text/markdown'), ('mfm', 'text/x.misskeymarkdown')]:
            if content_type in metadata.get('post_formats', []):
                ids[key] = post(main, '**compatibility ' + key + '**', content_type=content_type)
        if 'quote_posting' in metadata.get('features', []):
            ids['extension_quote'] = post(main, 'compatibility extension quote', quote_id=ids['text'])
        # This records an extension response; mstdn gains no reaction command.
        second.json('/api/v1/pleroma/statuses/' + ids['text'] + '/reactions/' + urllib.parse.quote('👍'), {}, method='PUT')
        ids['emoji_reaction'] = ids['text']
    ids['deleted'] = post(main, 'compatibility deleted')
    main.json('/api/v1/statuses/' + ids['deleted'], method='DELETE')
    for visibility in ('public', 'unlisted', 'private', 'direct'):
        ids[visibility] = post(main, '@bob compatibility ' + visibility, visibility=visibility, spoiler_text='compatibility warning')
    ids['wide'] = post(main, 'compatibility wide thread')
    for i in range(wide_count):
        post(second, f'@alice wide reply {i:03}', in_reply_to_id=ids['wide'])
    counts['wide_replies'] = wide_count
    ids['deep'] = post(main, 'compatibility deep thread')
    parent = ids['deep']
    for i in range(deep_count):
        parent = post(second, f'@alice deep reply {i:03}', in_reply_to_id=parent)
    ids['deep_last'] = parent
    counts['deep_replies'] = deep_count
    for i in range(mention_count):
        post(second, f'@alice notification {i:03}')
        if i % 100 == 99:
            print(f'Seeded {i+1} mention notifications', flush=True)
    counts['mention_notifications'] = mention_count
    for action in ('favourite', 'reblog'):
        second.json('/api/v1/statuses/' + ids['text'] + '/' + action, {})
    for i in range(100):
        media = first_video if i == 0 else upload(video, work / 'clip.mp4', f'Video {i:03}')
        post(video, f'video{i:03} #compatvideo', media_ids=[media])
        if i % 20 == 19:
            print(f'Seeded {i+1} video-only posts', flush=True)
    counts['video_only'] = 100
    for i in range(picture_count):
        media = [upload(second, work / 'picture.png', f'Picture {i:03}/{n}') for n in range(4)]
        post(second, f'pictures{i:03} #compatpictures', media_ids=media)
        if i % 20 == 19:
            print(f'Seeded {i+1} four-picture posts', flush=True)
    counts['four_pictures'] = picture_count
    for n in (1, 2, 3):
        media = [upload(main, work / 'picture.png', f'Alt {n}/{i}') for i in range(n)]
        ids['picture' + str(n)] = post(main, f'compatibility {n} pictures', media_ids=media)
    media = upload(main, work / 'unsupported.webm', 'Unsupported codec source')
    ids['unsupported_video'] = post(main, 'compatibility codec #compatcodec', media_ids=[media])
    # A server may transcode this to H.264; the stored type is recorded honestly.
    manifest['codec_attachment'] = main.json('/api/v1/statuses/' + ids['unsupported_video'])['media_attachments'][0]
    unicode_text = '👨‍👩‍👧 👍🏽 🇯🇵 ✈️ 漢字 العربية e\u0301 ' + 'longword' * 20
    ids['unicode'] = post(main, unicode_text, media_ids=[upload(main, work / 'picture.png', unicode_text)])
    main.json('/api/v1/accounts/update_credentials', {'display_name': 'Alice 👩🏽‍💻 🇯🇵 漢字 e\u0301'}, method='PATCH')
    if system == 'gotosocial':
        main.json('/api/v1/admin/custom_emojis', {'shortcode': 'compatblue'}, files=[('image', work / 'picture.png', 'image/png')])
        ids['emoji'] = post(main, 'compatibility :compatblue:')
    else:
        manifest['custom_emoji'] = 'not verified: this bootstrap does not provision custom emoji for this server'
    limits = manifest['limits']
    max_text = limits.get('statuses', {}).get('max_characters', 500)
    (work / 'max-text.txt').write_text('x' * max_text)
    (work / 'over-text.txt').write_text('x' * (max_text + 1))
    max_alt = limits.get('media_attachments', {}).get('description_limit', 1500)
    (work / 'max-alt.txt').write_text('a' * max_alt)
    (work / 'over-alt.txt').write_text('a' * (max_alt + 1))
    sized_videos(work, limits['media_attachments']['video_size_limit'], manifest['multipart_limit'])
    ids['max_text'] = post(main, 'x' * max_text)
    # Put the TUI smoke post first on home after the heavy data is ready.
    media = upload(main, work / 'clip.mp4', 'Compatibility video')
    ids['viewer_video'] = post(main, 'compatibility viewer video', media_ids=[media])
    media = upload(main, work / 'picture.png', 'Compatibility picture')
    ids['viewer_picture'] = post(main, 'compatibility viewer picture', media_ids=[media])
    counts['posts_deleted'] = 1
    (work / 'seed.json').write_text(json.dumps(manifest, indent=2))
    print(json.dumps({'seed_counts': counts, 'server_version': manifest['instance']['version']}), flush=True)
    return manifest


def main():
    work = Path(sys.argv[1])
    system = sys.argv[2]
    manifest = json.loads((work / 'seed.json').read_text()) if (work / 'seed.json').exists() else seed(work, system)
    capture(work, manifest)
    users = json.loads((work / 'users.json').read_text())
    env = dict(os.environ, MSTDN_TOKEN=users[0]['token'], MSTDN_TOKEN_SECOND=users[1]['token'], MSTDN_COMPAT_SEED=str(work), MSTDN_COMPAT_SYSTEM=system, MSTDN_COMPAT_ROOT=str(Path(__file__).resolve().parents[2]))
    for key, value in manifest['ids'].items():
        env['MSTDN_SEED_' + key.upper()] = value
    if manifest['codec_attachment'].get('url', '').endswith('.webm'):
        env['MSTDN_PRESERVES_WEBM'] = '1'
    env['MSTDN_COMPAT_' + system.upper()] = '1'
    env['MSTDN_HAS_QUOTES' if manifest['quote_posts'] else 'MSTDN_NO_QUOTES'] = '1'
    if not isinstance(manifest['accounts'][0].get('source', {}).get('quote_policy'), str):
        env['MSTDN_NO_QUOTE_POLICY'] = '1'
    env['MSTDN_QUOTE_PATH'] = {'akkoma': '$[0].quote_id', 'pleroma': '$[0].pleroma.quote_id'}.get(system, '$[0].quote.quoted_status.id')
    env['MSTDN_BLOCKED_PUBLIC' if system in ('akkoma', 'pleroma') else 'MSTDN_BLOCKED_HIDDEN'] = '1'
    env['MSTDN_SEED_ALICE'] = manifest['accounts'][0]['id']
    env['MSTDN_SEED_BOB'] = manifest['accounts'][1]['id']
    env['MSTDN_LIMIT_TEXT'] = str(manifest['limits']['statuses']['max_characters'])
    env['MSTDN_LIMIT_ALT'] = str(manifest['limits']['media_attachments'].get('description_limit', 1500))
    env['MSTDN_LIMIT_VIDEO'] = str(manifest['limits']['media_attachments']['video_size_limit'])
    env['MSTDN_LIMIT_VIDEO_MESSAGE'] = 'takes multipart uploads up to' if manifest['multipart_limit'] else 'takes files smaller than'
    sys.exit(subprocess.call([str(Path(__file__).with_name('run.sh'))], env=env))


if __name__ == '__main__':
    main()
