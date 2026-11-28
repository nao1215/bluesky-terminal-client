# Cryptographic fixtures

`nip59.json` contains the recipient private key and the rumor, seal and gift-wrap event from the official NIP-59 document's “An Example” section: https://github.com/nostr-protocol/nips/blob/master/59.md#an-example. It was retrieved on 2026-11-28. The JSON formatting was normalized and the example's trailing comma was removed; the event fields, ciphertexts, IDs and signatures are unchanged. These values were not generated for this project.

The official example wraps a kind-1 rumor. It is a NIP-59 wrapping fixture, not a kind-14 NIP-17 conversation vector. The NIP-17 sender-binding and round-trip tests generate their own test keys and messages separately.

`nip44-v2.json` embeds selected NIP-44 v2 reference vectors with their source recorded in the file. The tests check conversation keys, padding, authenticated encryption and malformed payloads without contacting any server.
