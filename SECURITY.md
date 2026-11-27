# Security Policy

## Reporting a Vulnerability

Please do not open a public issue for a vulnerability. Report it privately through [GitHub's private vulnerability reporting](https://github.com/nao1215/misskey-terminal-client/security/advisories/new) (the Security tab, then "Report a vulnerability"), or by mail to [n.chika156@gmail.com](mailto:n.chika156@gmail.com). We appreciate your responsible disclosure and will work with you to address the issue promptly.

## Verifying a Release

Each release attaches `checksums.txt`, its cosign signature (`checksums.txt.sigstore.json`), SLSA provenance (`msky-<tag>.intoto.jsonl`) and a CycloneDX SBOM, and every archive has a GitHub build provenance attestation. With the files of a release in one directory:

```sh
sha256sum --check --ignore-missing checksums.txt
cosign verify-blob --bundle checksums.txt.sigstore.json \
  --certificate-identity "https://github.com/nao1215/misskey-terminal-client/.github/workflows/release.yml@refs/tags/<tag>" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com checksums.txt
slsa-verifier verify-artifact msky-<tag>-<target>.tar.gz --provenance-path msky-<tag>.intoto.jsonl \
  --source-uri github.com/nao1215/misskey-terminal-client --source-tag <tag>
gh attestation verify msky-<tag>-<target>.tar.gz --repo nao1215/misskey-terminal-client
```

## Supported Versions

We recommend using the latest release for the most up-to-date and secure experience. Security updates are provided for the latest stable version.

## Security Policy

- Security issues are treated with the highest priority.
- We follow responsible disclosure practices.
- Fixes for security vulnerabilities will be provided in a timely manner.

## Acknowledgments

We would like to thank the security researchers and contributors who responsibly report security issues and work with us to make our project more secure.

Thank you for your help in making our project safe and secure for everyone.
