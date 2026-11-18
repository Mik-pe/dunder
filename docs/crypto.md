# Native cryptography

`std.crypto` provides bounded random bytes, Argon2id password hashes,
HMAC-SHA256, HKDF-SHA256, constant-time byte equality, hex decoding and
AES-256-GCM authenticated encryption. Native random, Argon2id and AES use
the installed system libraries through typed C FFI. HMAC and HKDF build
on Dunder's SHA-256 implementation.

`random(count)` returns `BytesResult { ok, value, error }`, with at most
1 MiB per call. `argon2id_hash(password)` returns `TextResult` containing
a PHC string with time cost 2, memory cost 19,456 KiB, one lane, a random
16-byte salt and a 32-byte hash. `argon2id_verify(password, phc)` accepts
Argon2id version 19 and bounds memory, iterations and parallelism before
calling the library. Password lengths are explicit, including embedded NUL.

`hmac_sha256(&key, &data)` returns 32 bytes.
`hkdf_sha256(&key, &salt, &info, length)` implements RFC 5869 and returns
at most 8,160 bytes. Empty salt has the RFC-defined zero-key behavior.
`equal(&left, &right)` compares equal-length bytes without data-dependent
early exits. Lengths remain observable.

`aes256_gcm_seal(&key, &nonce, &aad, &plaintext)` returns
`SealResult { ok, ciphertext, tag, error }`. The key is 32 bytes, nonce
12 bytes, and tag 16 bytes. Generate a fresh random nonce for each
encryption under the same key. Plaintext/ciphertext is bounded to 16 MiB,
and associated data to 1 MiB. `aes256_gcm_open` takes those same inputs
plus `&tag` and returns a `BytesResult`. It returns no plaintext when
authentication fails. Callers own nonce uniqueness and key management.

macOS dependencies are Homebrew `openssl@3` and `argon2`, loaded from their
stable `/opt/homebrew/opt/` library paths. Linux uses `libcrypto.so.3` and
`libargon2.so.1`. No compiler, subprocess or application shim implements
the cryptographic operations.

Run `/absolute/path/to/dunder run tools/test_crypto.ddr -- /absolute/path/to/dunder`. Its native
fixture checks published NIST AES-256-GCM and RFC HMAC/HKDF vectors,
authentication failures, zero-length inputs, bounds, password verification
and random generation. Additional runtimes can be selected with
`--runner TARGET=COMMAND`.
