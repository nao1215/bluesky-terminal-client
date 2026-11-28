# Exit statuses

These meanings are shared by `bsky`, `msky`, `mstdn` and `nstr`. A client uses a status when it implements that failure class; this does not assert that every client implements every feature.

| Status | Meaning |
| --- | --- |
| 0 | Success |
| 1 | Usage error, including locally detected missing credentials or insufficient saved scopes |
| 2 | Terminal error |
| 3 | Local file or output error |
| 4 | Network or server error, including HTTP authentication and permission refusals |
| 5 | The server does not offer the requested feature |

Status 5 names both the feature and server. It requires endpoint or capability evidence; a failed connection, HTTP 401/403, rate limit, missing resource or hidden object is not evidence of an unsupported feature. With `--json`, the error's `status` agrees with the process exit status. In this branch mstdn implements status 5; other clients retain their existing implementations.
