#!/usr/bin/env bash
# Official Mastodon compose, with throwaway volumes and a loopback HTTP origin.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
umask 077
work="$(mktemp -d "${TMPDIR:-/tmp}/stc-masto.XXXXXXXX")"
name="$(basename "$work" | tr '[:upper:].' '[:lower:]-')"
compose=(docker compose --project-name "$name" --project-directory "$work" -f "$work/compose.yml" -f "$ROOT/scripts/compat/mastodon.override.yml")
cleanup() {
    if [[ -f "$work/.env.production" ]]; then
        "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
    fi
    python3 -c 'import shutil,sys; shutil.rmtree(sys.argv[1])' "$work"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
export MSTDN_HOST=http://127.0.0.1:13000
curl --fail --silent --show-error --max-time 60 https://raw.githubusercontent.com/mastodon/mastodon/v4.5.0/docker-compose.yml -o "$work/compose.yml"
python3 - "$work" <<'PY'
import hashlib, pathlib, secrets, sys
work = pathlib.Path(sys.argv[1])
assert hashlib.sha256((work / 'compose.yml').read_bytes()).hexdigest() == 'd2c6af546d72e3ad53962289a85f745f2257f6e7ecc1f0b6bebde55cc551cc6c'
settings = dict(LOCAL_DOMAIN='127.0.0.1:13000', DB_HOST='db', DB_USER='postgres', DB_PASS='', DB_NAME='mastodon', REDIS_HOST='redis', RAILS_ENV='production', RAILS_SERVE_STATIC_FILES='true', RAILS_LOG_TO_STDOUT='true', SMTP_SERVER='127.0.0.1', SMTP_PORT='25', EMAIL_DOMAIN_ALLOWLIST='example.invalid', WEB_CONCURRENCY='0', MAX_THREADS='5', SIDEKIQ_CONCURRENCY='5', S3_ENABLED='false', AUTHORIZED_FETCH='false', LIMITED_FEDERATION_MODE='true')
for key in ('SECRET_KEY_BASE', 'OTP_SECRET', 'ACTIVE_RECORD_ENCRYPTION_PRIMARY_KEY', 'ACTIVE_RECORD_ENCRYPTION_DETERMINISTIC_KEY', 'ACTIVE_RECORD_ENCRYPTION_KEY_DERIVATION_SALT'):
    settings[key] = secrets.token_hex(32)
(work / '.env.production').write_text(''.join(f'{k}={v}\n' for k,v in settings.items()))
# Test deployment configuration: production normally requires HTTPS. This
# initializer changes transport only, so no test CA or TLS bypass enters mstdn.
(work / 'local_http.rb').write_text('Rack::Attack.enabled = false\nRails.application.config.force_ssl = false\nRails.application.config.x.use_https = false\nRails.application.config.action_mailer.default_url_options[:protocol] = "http://"\n# The catalog needs more than the default 300 posts per account per 3 hours.\n# Only the disposable deployment changes this limit; seeding still uses HTTP.\nRails.application.config.after_initialize do\n  families = RateLimiter::FAMILIES.merge(statuses: { limit: 10000, period: 3.hours }.freeze).freeze\n  RateLimiter.send(:remove_const, :FAMILIES)\n  RateLimiter.const_set(:FAMILIES, families)\nend\n')
(work / 'local_http.rb').chmod(0o644)
PY
"${compose[@]}" pull db redis web sidekiq >/dev/null
"${compose[@]}" up -d db redis
"${compose[@]}" run --rm web bundle exec rails db:prepare > "$work/migrate.log" 2>&1
"${compose[@]}" run --rm web bundle exec rake mastodon:webpush:generate_vapid_key >> "$work/.env.production"
for user in alice bob videos; do
    role=()
    if [[ "$user" == alice ]]; then role=(--role Owner); fi
    "${compose[@]}" run --rm web bin/tootctl accounts create "$user" --email "$user@example.invalid" --confirmed --approve "${role[@]}" > "$work/$user.txt"
done
python3 - "$work" <<'PY'
import json, pathlib, re, sys
work = pathlib.Path(sys.argv[1]); users = []
for name in ('alice', 'bob', 'videos'):
    text = (work / (name + '.txt')).read_text()
    password = re.search(r'New password: (\S+)', text)
    if not password: raise RuntimeError('tootctl did not return a password')
    users.append(dict(username=name, email=name+'@example.invalid', password=password[1]))
(work / 'users.json').write_text(json.dumps(users))
PY
"${compose[@]}" up -d web sidekiq
python3 "$ROOT/scripts/compat/server.py" wait "$MSTDN_HOST"
python3 "$ROOT/scripts/compat/server.py" authorize "$MSTDN_HOST" mastodon "$work"
export MSTDN_COMPAT_CONTAINER
MSTDN_COMPAT_CONTAINER="$("${compose[@]}" ps -q web)"
python3 "$ROOT/scripts/compat/seed.py" "$work" mastodon
