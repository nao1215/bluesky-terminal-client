# atago Behavior Specs
## Summary
1 suite · 9 scenarios
## Contents
- [bsky videos](#bsky-videos): 9 scenarios
  - [a video is posted alone, with its alt text and shape](#scenario-a-video-is-posted-alone-with-its-alt-text-and-shape)
  - [a QuickTime video without an ftyp box is posted as one](#scenario-a-quicktime-video-without-an-ftyp-box-is-posted-as-one)
  - [an animated GIF goes to the video service as it is](#scenario-an-animated-gif-goes-to-the-video-service-as-it-is)
  - [a video the service cannot process is reported and nothing is posted](#scenario-a-video-the-service-cannot-process-is-reported-and-nothing-is-posted)
  - [an account Bluesky will not take videos from says why before anything is sent](#scenario-an-account-bluesky-will-not-take-videos-from-says-why-before-anything-is-sent)
  - [pictures and a video do not go on one post](#scenario-pictures-and-a-video-do-not-go-on-one-post)
  - [a video on the timeline is shown by its thumbnail](#scenario-a-video-on-the-timeline-is-shown-by-its-thumbnail)
  - [a video longer than three minutes is refused with its length, before anything is sent](#scenario-a-video-longer-than-three-minutes-is-refused-with-its-length-before-anything-is-sent)
  - [a video service typed on the settings screen is where the video goes](#scenario-a-video-service-typed-on-the-settings-screen-is-where-the-video-goes)

## bsky videos
Source: `crates/bluesky-terminal-client/e2e/atago/videos.atago.yaml`
### Scenario: a video is posted alone, with its alt text and shape
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/clip.mp4` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/app.bsky.video.getUploadLimits` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.server.getServiceAuth` exactly 2 time(s)
- mock `api` received `/xrpc/com.atproto.server.getServiceAuth`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.video.uploadVideo`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo`
- mock `api` received `/xrpc/app.bsky.video.getJobStatus`
- mock `api` received `/xrpc/com.atproto.repo.uploadBlob` exactly 0 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: a QuickTime video without an ftyp box is posted as one
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/classic.mov` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.video.uploadVideo`
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)

### Scenario: an animated GIF goes to the video service as it is
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/moving.gif` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord`

### Scenario: a video the service cannot process is reported and nothing is posted
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/clip.mp4` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: an account Bluesky will not take videos from says why before anything is sent
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/clip.mp4` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/com.atproto.server.getServiceAuth` exactly 1 time(s)
- mock `api` received `/xrpc/app.bsky.video.uploadVideo` exactly 0 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: pictures and a video do not go on one post
#### Given
- Stub HTTP server `api` serves 1 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/clip.mp4` is created.
- Fixture file `media/photo.png` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`

### Scenario: a video on the timeline is shown by its thumbnail
#### Given
- Stub HTTP server `api` serves 2 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- rendered screen shows exactly 1 image(s) and an image width 320px, height 240px, like testdata/expected/photo-320x240.png
- mock `api` received `/img/thumb.png` exactly 1 time(s)
- mock `api` received `/video/playlist.m3u8` exactly 0 time(s)

### Scenario: a video longer than three minutes is refused with its length, before anything is sent
#### Given
- Stub HTTP server `api` serves 7 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/too-long.mp4` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo` exactly 0 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 0 time(s)

### Scenario: a video service typed on the settings screen is where the video goes
#### Given
- Stub HTTP server `api` serves 9 canned route(s) at `${api.url}` and records every request (#24).
- Fixture file `cfg/accounts/did_plc_me.json` is created.
- Fixture file `media/clip.mp4` is created.

#### Inputs
_Fixture `cfg/accounts/did_plc_me.json`:_

```text
{"service": "${api.url}", "did": "did:plc:me", "handle": "me.test", "accessJwt": "acc", "refreshJwt": "ref"}
```

#### When

```shell
# interactive (pty): bsky
```

#### Then
- exit code is `0`
- file `cfg/settings.json` at `$.video_service` equals `${api.url}`
- mock `api` received `/xrpc/app.bsky.video.uploadVideo` exactly 1 time(s)
- mock `api` received `/xrpc/com.atproto.repo.createRecord` exactly 1 time(s)
