#!/usr/bin/env python3
"""Provision throwaway loopback servers using their public HTTP interfaces."""
from html.parser import HTMLParser
import http.cookiejar
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request


class Forms(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.forms = []
        self.current = None
        self.codes = []
        self.in_code = False
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == 'form':
            self.current = {'action': attrs.get('action', ''), 'fields': {}}
            self.forms.append(self.current)
        if tag in ('input', 'button') and self.current is not None and attrs.get('name'):
            if attrs.get('type') not in ('checkbox', 'submit') or 'checked' in attrs:
                self.current['fields'][attrs['name']] = attrs.get('value', '')
        if tag == 'input' and 'oauth-code' in attrs.get('class', '').split():
            self.codes.append(attrs.get('value', ''))
        if tag == 'code':
            self.in_code = True

    def handle_endtag(self, tag):
        if tag == 'form':
            self.current = None
        if tag == 'code':
            self.in_code = False

    def handle_data(self, data):
        if self.in_code:
            self.codes.append(data.strip())


class LocalRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).netloc != urllib.parse.urlsplit(req.full_url).netloc:
            raise RuntimeError('refusing a redirect outside the local server')
        return super().redirect_request(req, fp, code, msg, headers, newurl)


class API:
    def __init__(self, host, token=None, timeout=60):
        parsed = urllib.parse.urlsplit(host)
        if parsed.scheme != 'http' or parsed.hostname != '127.0.0.1' or parsed.username or parsed.path:
            raise ValueError('only http://127.0.0.1:PORT origins are permitted')
        self.host = host
        self.token = token
        self.timeout = timeout
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), LocalRedirect(), urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def request(self, path, data=None, method=None, form=False, files=None, allow_error=False):
        url = urllib.parse.urljoin(self.host, path)
        if urllib.parse.urlsplit(url).netloc != urllib.parse.urlsplit(self.host).netloc:
            raise ValueError('refusing a request outside the local server')
        headers = {}
        if self.token:
            headers['Authorization'] = 'Bearer ' + self.token
        if files:
            boundary = secrets.token_hex(16)
            body = bytearray()
            for name, value in (data or {}).items():
                body.extend(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode())
            for name, file_path, mime in files:
                body.extend(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"; filename="{file_path.name}"\r\nContent-Type: {mime}\r\n\r\n'.encode())
                body.extend(file_path.read_bytes())
                body.extend(b'\r\n')
            body.extend(f'--{boundary}--\r\n'.encode())
            headers['Content-Type'] = 'multipart/form-data; boundary=' + boundary
            data = bytes(body)
        elif data is not None:
            headers['Content-Type'] = 'application/x-www-form-urlencoded' if form else 'application/json'
            data = (urllib.parse.urlencode(data) if form else json.dumps(data)).encode()
        req = urllib.request.Request(url, data, headers, method=method)
        try:
            with self.opener.open(req, timeout=self.timeout) as response:
                self.response = {'status': response.status, 'link': response.headers.get('Link')}
                return response.read(), response.geturl()
        except urllib.error.HTTPError as error:
            if allow_error:
                self.response = {'status': error.code, 'link': error.headers.get('Link')}
                return error.read(), error.geturl()
            # Never log an OAuth body, a password, a token or form input.
            raise RuntimeError(f'{method or ("POST" if data is not None else "GET")} {path.split("?")[0]}: HTTP {error.code}') from None

    def json(self, path, data=None, **kwargs):
        return json.loads(self.request(path, data, **kwargs)[0])


def authorize(host, email, password, scopes, password_grant=False):
    api = API(host)
    redirect = 'urn:ietf:wg:oauth:2.0:oob'
    app = api.json('/api/v1/apps', {'client_name': 'mstdn local compatibility', 'redirect_uris': redirect, 'scopes': scopes})
    if password_grant:
        # Pleroma.Web.OAuth.OAuthController implements this grant in both
        # projects. It authenticates only these newly created local users.
        return api.json('/oauth/token', {'grant_type': 'password', 'client_id': app['client_id'], 'client_secret': app['client_secret'], 'username': email, 'password': password, 'scope': scopes})['access_token']
    query = urllib.parse.urlencode({'client_id': app['client_id'], 'redirect_uri': redirect, 'response_type': 'code', 'scope': scopes})
    html, url = api.request('/oauth/authorize?' + query)
    for _ in range(6):
        page = Forms(html.decode())
        candidates = [f for f in page.forms if 'password' in f['fields'] or 'user[password]' in f['fields']]
        if candidates:
            f = candidates[0]
            fields = f['fields']
            if 'user[password]' in fields:
                fields.update({'user[email]': email, 'user[password]': password})
            else:
                fields.update({'username': email, 'password': password})
        else:
            candidates = [f for f in page.forms if f['action'].endswith('/oauth/authorize') and f['fields'].get('_method') != 'delete']
            if not candidates:
                codes = [c for c in page.codes if c and not c.startswith('urn:')]
                if len(codes) != 1:
                    raise RuntimeError('OAuth did not return exactly one authorization code')
                return api.json('/oauth/token', {'grant_type': 'authorization_code', 'client_id': app['client_id'], 'client_secret': app['client_secret'], 'redirect_uri': redirect, 'code': codes[0]})['access_token']
            f = candidates[0]
            fields = f['fields']
        html, url = api.request(urllib.parse.urljoin(url, f['action']), fields, form=True)
    raise RuntimeError('OAuth form flow did not finish')


def wait(host, authenticated=False, container=None):
    api = API(host, os.environ['MSTDN_TOKEN'] if authenticated else None, timeout=2)
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        if container:
            state = subprocess.run(['docker', 'inspect', '--format', '{{.State.Running}}', container], check=True, capture_output=True, text=True)
            if state.stdout.strip() != 'true':
                raise RuntimeError('server container stopped before becoming ready')
        try:
            api.json('/api/v1/instance')
            if authenticated:
                api.json('/api/v1/timelines/home?limit=1')
            return
        except (RuntimeError, urllib.error.URLError, TimeoutError, ConnectionError):
            time.sleep(1)
    raise RuntimeError('server did not become ready within 120 seconds')


def users_gts(container, work):
    users = []
    for name in ('alice', 'bob', 'videos'):
        password = secrets.token_urlsafe(24)
        email = name + '@example.invalid'
        subprocess.run(['docker', 'exec', container, '/gotosocial/gotosocial', 'admin', 'account', 'create', '--username', name, '--email', email, '--password', password], check=True, stdout=subprocess.DEVNULL)
        users.append({'username': name, 'email': email, 'password': password})
    subprocess.run(['docker', 'exec', container, '/gotosocial/gotosocial', 'admin', 'account', 'promote', '--username', 'alice'], check=True, stdout=subprocess.DEVNULL)
    (work / 'users.json').write_text(json.dumps(users))


def config_pleroma(work, port):
    secret = secrets.token_hex(64)
    config = f'''import Config
config :pleroma, Pleroma.Web.Endpoint,
  url: [host: "127.0.0.1", scheme: "http", port: {port}],
  http: [ip: {{0, 0, 0, 0}}, port: 4000],
  secret_key_base: "{secret}", signing_salt: "compatibility"
config :pleroma, Pleroma.Repo,
  username: "pleroma", password: "compat", database: "pleroma", hostname: "db", pool_size: 10
config :pleroma, :instance,
  name: "mstdn disposable compatibility", email: "admin@example.invalid",
  notify_email: "admin@example.invalid", registrations_open: false,
  federating: false, healthcheck: true, static_dir: "/var/lib/pleroma/static"
config :pleroma, :database, rum_enabled: false
config :pleroma, :rate_limit,
  authentication: nil, timeline: nil, search: nil, app_account_creation: nil,
  relations_actions: nil, relation_id_action: nil, statuses_actions: nil,
  status_id_action: nil, password_reset: nil, account_confirmation_resend: nil, ap_routes: nil
config :pleroma, Pleroma.Uploaders.Local, uploads: "/var/lib/pleroma/uploads"
config :pleroma, Pleroma.Upload, base_url: "http://127.0.0.1:{port}/media/"
config :pleroma, Pleroma.Emails.Mailer, enabled: false
config :pleroma, :rich_media, enabled: false
config :pleroma, :media_proxy, enabled: false
config :pleroma, :shout, enabled: false
config :logger, level: :warning
config :tzdata, :autoupdate, :disabled
'''
    (work / 'config.exs').write_text(config)
    (work / 'config.exs').chmod(0o600)


def users_pleroma(container, work):
    command = ['/opt/pleroma/bin/pleroma_ctl', 'user']
    users = []
    for name in ('alice', 'bob', 'videos'):
        password = 'compat-' + secrets.token_urlsafe(24)
        email = name + '@example.invalid'
        args = ['docker', 'exec', container, *command, 'new', name, email, '--password', password, '--assume-yes']
        if name == 'alice':
            args.append('--admin')
        # The admin tool echoes its arguments, including the password.
        result = subprocess.run(args, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if result.returncode:
            raise RuntimeError('admin account creation failed for ' + name)
        users.append({'username': name, 'email': email, 'password': password})
    (work / 'users.json').write_text(json.dumps(users))


def main():
    mode = sys.argv[1]
    if mode in ('wait', 'ready'):
        wait(sys.argv[2], authenticated=mode == 'ready', container=sys.argv[3] if len(sys.argv) > 3 else None)
    elif mode in ('stop', 'start'):
        containers = [os.environ['MSTDN_COMPAT_CONTAINER']]
        proxy = os.environ.get('MSTDN_COMPAT_PROXY')
        if proxy:
            containers.append(proxy)
        if mode == 'stop':
            containers.reverse()
        for container in containers:
            subprocess.run(['docker', mode, container], check=True, stdout=subprocess.DEVNULL)
    elif mode == 'users-gts':
        users_gts(sys.argv[2], Path(sys.argv[3]))
    elif mode == 'config-pleroma':
        config_pleroma(Path(sys.argv[2]), int(sys.argv[3]))
    elif mode == 'users-pleroma':
        users_pleroma(sys.argv[2], Path(sys.argv[3]))
    elif mode == 'authorize':
        host, system, directory = sys.argv[2:]
        work = Path(directory)
        users = json.loads((work / 'users.json').read_text())
        for user in users:
            scopes = 'read write'
            if user['username'] == 'alice' and system in ('gotosocial', 'mastodon'):
                scopes += ' admin' if system == 'gotosocial' else ' admin:read admin:write'
            user['token'] = authorize(host, user['email'], user['password'], scopes, password_grant=system in ('akkoma', 'pleroma'))
            print('Authorized throwaway account ' + user['username'], flush=True)
        (work / 'users.json').write_text(json.dumps(users))
    else:
        raise ValueError('unknown mode')


if __name__ == '__main__':
    main()
