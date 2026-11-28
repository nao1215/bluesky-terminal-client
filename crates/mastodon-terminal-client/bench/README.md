# Benchmarks

mstdn is measured the way a user runs it, one process per call from start to exit, with [himorime](https://github.com/nao1215/himorime). himorime builds mstdn, runs commands in interleaved rounds, and reports latency, CPU time and peak RSS.

```console
$ himorime run bench
$ himorime compare --against main bench
$ himorime run --filter 'client first post' bench
```

The suite needs Python 3 and atago (CI uses v0.27.1). Every endpoint is on 127.0.0.1 and every configuration directory is temporary; no account or public server is used. `serve.sh` starts `mock.py` before command measurements and stops its process group afterward. The mock uses atago routes and the existing E2E public-100.json fixture, including HTML, Japanese and emoji.

| Benchmark | Commands | Measures |
|---|---|---|
| `version` | `mstdn --version` | process start-up |
| `timeline 100` | `mstdn timeline -n 100`, with and without `--json` | reading and printing 100 posts |
| `notifications 100` | `mstdn notifications -n 100` | reading and printing 100 notifications |
| `thread 100` | `mstdn thread 200` | a status and 100 replies |
| `search 100` | `mstdn search rust -n 100` | 100 statuses found |
| `client first post` | `mstdn` on a pseudo-terminal through `first_post.py` | first post drawn, then `q`; instant responses and terminal/server delays of 150/300 ms |

`first_post.py` uses the shared probe in `scripts/bench/runner.py`. The measured Python process starts a one-post stand-in, starts the client in a 120 by 40 pseudo-terminal, answers terminal queries, waits for `FIRSTPOSTMARK`, sends `q`, and requires a successful exit within the deadline. Python and fixture start-up (including atago) are included in these numbers. The instant variant has no injected delay; the over-ssh variant delays terminal replies by 150 ms and every server request by 300 ms.

The suite uses two warmups and 20 measured runs. On pull requests, `.github/workflows/bench.yml` compares the base and head in interleaved rounds on one runner at 95% confidence: latency allows 10% with a 2 ms minimum difference, CPU time 15% with 2 ms, and peak RSS 15% with 2 MiB. Inconclusive comparisons do not fail the job. A manual run, or a base without the suite, measures only the current revision.

Numbers from different machines are not comparable; compare revisions on one machine.
