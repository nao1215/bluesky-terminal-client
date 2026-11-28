# atago Behavior Specs
## Summary
1 suite · 1 scenario
## Contents
- [nstr parity protocol](#nstr-parity-protocol): 1 scenario
  - [exact signed writes and read commands on local relays](#scenario-exact-signed-writes-and-read-commands-on-local-relays)

## nstr parity protocol
Source: `crates/nostr-terminal-client/e2e/atago/parity.atago.yaml`
### Scenario: exact signed writes and read commands on local relays
#### Given
- Background service `relay` is started: `python3 "${specdir}/testdata/relay.py" --parity`.
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- Environment variables are set: NSTR_CONFIG_DIR.
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).
- The command runs with an isolated home under `${workdir}/.atago-home` (HOME/XDG or APPDATA redirected).

#### When

```shell
nstr profile 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json
nstr author 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 -n 2 --json
nstr note 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
nstr thread 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
nstr search bob --json
nstr like 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
python3 "${specdir}/testdata/check_parity.py" 7
nstr unlike 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
python3 "${specdir}/testdata/check_parity.py" 5
nstr repost 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
python3 "${specdir}/testdata/check_parity.py" 6
nstr unrepost 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
python3 "${specdir}/testdata/check_parity.py" 5
nstr follow 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json
python3 "${specdir}/testdata/check_parity.py" 3
nstr unfollow 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json
python3 "${specdir}/testdata/check_parity.py" 3
nstr mute 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json
python3 "${specdir}/testdata/check_parity.py" 10000
nstr mutes --json
nstr unmute 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json
nstr report 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --reason spam --comment "fixture report" --yes --json
python3 "${specdir}/testdata/check_parity.py" 1984
nstr profile-update '{"name":"updated"}'  --json
nstr dm-relays --json
python3 "${specdir}/testdata/check_parity.py" 10050
nstr follows --json
nstr followers --json
nstr likes 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
nstr reposts 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json
nstr post "quote" --quote 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --cw spoiler --json
python3 "${specdir}/testdata/check_parity.py" 1
nstr delete 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --yes --json
python3 "${specdir}/testdata/check_parity.py" 5
nstr accounts --json
nstr chat 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 "secret fixture message" --json
python3 "${specdir}/testdata/check_dm.py"
```

#### Then
- after `nstr profile 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json`:
  - exit code is `0`
- after `nstr author 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 -n 2 --json`:
  - exit code is `0`
- after `nstr note 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `nstr thread 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `nstr search bob --json`:
  - exit code is `0`
- after `nstr like 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 7`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr unlike 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 5`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr repost 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 6`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr unrepost 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 5`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr follow 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 3`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr unfollow 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 3`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr mute 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 10000`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr mutes --json`:
  - exit code is `0`
- after `nstr unmute 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 --json`:
  - exit code is `0`
- after `nstr report 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --reason spam --comment "fixture report" --yes --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 1984`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr profile-update '{"name":"updated"}'  --json`:
  - exit code is `0`
- after `nstr dm-relays --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 10050`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr follows --json`:
  - exit code is `0`
- after `nstr followers --json`:
  - exit code is `0`
- after `nstr likes 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `nstr reposts 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --json`:
  - exit code is `0`
- after `nstr post "quote" --quote 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --cw spoiler --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 1`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr delete 99570b44a0c1be6d814417bb5c02abec44d32c4c25fac68a11a304a98ed53662 --yes --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_parity.py" 5`:
  - exit code is `0`
  - stdout contains `exact event verified`
- after `nstr accounts --json`:
  - exit code is `0`
- after `nstr chat 27f97c0733efea6a7afd71aafa9ca10982982274ced13edcdac65e5b632bb589 "secret fixture message" --json`:
  - exit code is `0`
- after `python3 "${specdir}/testdata/check_dm.py"`:
  - exit code is `0`
  - stdout contains `gift wraps verified independently`
