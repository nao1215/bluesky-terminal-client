<!-- ALL-CONTRIBUTORS-BADGE:START - Do not remove or modify this section -->
[![All Contributors](https://img.shields.io/badge/all_contributors-1-orange.svg?style=flat-square)](#contributors)
<!-- ALL-CONTRIBUTORS-BADGE:END -->
[![CI](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/rust.yml)
[![E2E](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/e2e.yml)
![Coverage](https://raw.githubusercontent.com/nao1215/octocovs-central-repo/main/badges/nao1215/social-terminal-clients/coverage.svg)
[![Gitleaks](https://github.com/nao1215/social-terminal-clients/actions/workflows/gitleaks.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/gitleaks.yml)
[![Trivy](https://github.com/nao1215/social-terminal-clients/actions/workflows/trivy.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/trivy.yml)
[![cargo-deny](https://github.com/nao1215/social-terminal-clients/actions/workflows/cargo-deny.yml/badge.svg)](https://github.com/nao1215/social-terminal-clients/actions/workflows/cargo-deny.yml)
[![tested with atago](https://img.shields.io/badge/tested%20with-atago-7c3aed?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0iI2ZmZiIgZD0iTTMuNiA0LjIgMTEuOSAxMmwtOC4zIDcuOC0xLjktMi4yTDcuOSAxMiAxLjcgNi40eiIvPjxyZWN0IGZpbGw9IiNmZmYiIHg9IjEyLjYiIHk9IjE3LjIiIHdpZHRoPSI5LjciIGhlaWdodD0iMi44IiByeD0iMS40Ii8%2BPC9zdmc%2B&logoColor=white)](https://github.com/nao1215/atago)
[![measured with himorime](https://img.shields.io/badge/measured%20with-himorime-d9480f?logo=data:image/svg%2Bxml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI%2BPHBhdGggZmlsbD0ibm9uZSIgc3Ryb2tlPSIjZmZmIiBzdHJva2Utd2lkdGg9IjIuNCIgc3Ryb2tlLWxpbmVjYXA9InJvdW5kIiBkPSJNNC4yIDE4LjVBOSA5IDAgMSAxIDE5LjggMTguNSIvPjxwYXRoIGZpbGw9Im5vbmUiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIyLjQiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIgZD0iTTEyIDE0LjUgMTYuNSA5Ii8%2BPGNpcmNsZSBmaWxsPSIjZmZmIiBjeD0iMTIiIGN5PSIxNC41IiByPSIyLjIiLz48L3N2Zz4=&logoColor=white)](https://github.com/nao1215/himorime)
[![GitHub Downloads (all assets, all releases)](https://img.shields.io/github/downloads/nao1215/social-terminal-clients/total)](https://github.com/nao1215/social-terminal-clients/releases)
[![License](https://img.shields.io/github/license/nao1215/social-terminal-clients)](./LICENSE)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/nao1215/social-terminal-clients/badge)](https://scorecard.dev/viewer/?uri=github.com/nao1215/social-terminal-clients)

# social-terminal-clients

Terminal clients for social networks. Each client is a crate in `crates/`, installed and released on its own.

- [bsky](crates/bluesky-terminal-client/README.md): a client for Bluesky.
- [msky](crates/misskey-terminal-client/README.md): a client for Misskey.

The crate `crates/terminal-kit` holds the terminal drawing code both clients use. See [CONTRIBUTING.md](CONTRIBUTING.md) for working on the workspace.

## Contributors

Everyone who reports a bug, suggests a feature, sends a pull request, or packages a client is listed here ([emoji key](https://allcontributors.org/docs/en/emoji-key)).

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://debimate.jp/"><img src="https://avatars.githubusercontent.com/u/22737008?v=4?s=75" width="75px;" alt="CHIKAMATSU Naohiro"/><br /><sub><b>CHIKAMATSU Naohiro</b></sub></a><br /><a href="https://github.com/nao1215/social-terminal-clients/commits?author=nao1215" title="Code">💻</a> <a href="https://github.com/nao1215/social-terminal-clients/commits?author=nao1215" title="Documentation">📖</a></td>
    </tr>
  </tbody>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

[MIT](LICENSE). Not made by or affiliated with Bluesky Social PBC or the Misskey project.
