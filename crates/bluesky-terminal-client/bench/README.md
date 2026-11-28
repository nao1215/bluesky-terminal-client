# Benchmarks

bsky is measured the way a user runs it, one process per call from start to exit, with [himorime](https://github.com/nao1215/himorime). himorime builds bsky, runs each command in interleaved rounds, and reports latency, CPU time and peak RSS.

```console
$ go install github.com/nao1215/himorime@latest
$ himorime run bench                       # measure the working tree
$ himorime compare --against main bench    # compare main with the working tree
$ himorime run --filter 'first post' bench # one benchmark
```

No benchmark touches the network. The commands talk to `mock.py`, a stand-in server on 127.0.0.1 that `serve.sh` starts for each benchmark and stops after it; it answers at once with pages of 100 posts of mixed text (Japanese, emoji, links), so what is measured is bsky's own work.

On a pull request, `.github/workflows/bench.yml` runs `himorime ci bench`: the base of the pull request and its head are built and measured in the same rounds on one runner. The job fails when a command is slower, uses more CPU time or more memory than the base beyond the tolerance in `himorime.yaml` with 95% confidence. A difference too close to call is reported as inconclusive and does not fail the job.

| Benchmark | Commands | Measures |
|-----------|----------|----------|
| `version` | `bsky --version` | starting bsky |
| `timeline 100` | `bsky tl -n 100`, and with `--json` | reading a page of 100 posts and printing it |
| `notifications 100` | `bsky notif -n 100` | 100 notifications and the post they are about |
| `thread 100` | `bsky thread` | a post and its 100 replies |
| `search 100` | `bsky search -n 100` | 100 posts found |
| `client first post` | `bsky` on a pseudo-terminal, through `first_post.py` | from start to the first post on screen, then `q`: with a terminal and a server that answer at once, and with a terminal that answers in 150 ms (as over ssh) and a server in 300 ms |

`first_post.py` is the measured process of the last benchmark: it starts the client on a pseudo-terminal, answers the client's questions about what the terminal can draw after the given delay, serves the stand-in data after its own delay, and presses `q` when the first post is drawn. Its own start-up (Python) is in the numbers; compare revisions, not the absolute values.

Numbers from different machines are not comparable; compare revisions on one machine, as `himorime compare` and CI do.
