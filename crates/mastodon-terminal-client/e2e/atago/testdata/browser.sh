#!/bin/sh
# A stand-in for the browser of `mstdn login`: given the server's page, it
# comes back to where the page says (redirect_uri) with the state it was
# sent and a code, as the server sends the browser back once the access is
# allowed.
url="$1"
param() {
	printf '%s\n' "$url" | sed -n "s/.*[?&]$1=\([^&]*\).*/\1/p"
}
back=$(param redirect_uri | sed 's/%3A/:/g; s/%2F/\//g')
state=$(param state)
curl -s "$back?code=e2e-loopback-code&state=$state" >/dev/null
