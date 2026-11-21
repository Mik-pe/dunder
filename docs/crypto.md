# Native cryptography

`std.crypto` provides bounded random bytes, Argon2id password hashes,
HMAC-SHA256, HKDF-SHA256, constant-time byte equality, hex decoding,
AES-256-GCM authenticated encryption, Ed25519 signatures, and explicit
secret zeroization (`zeroize`, `SecretBytes`). Native random, Argon2id,
AES, and Ed25519 load the installed system libraries dynamically at runtime
without hardcoded compiler paths. HMAC and HKDF build on Dunder's SHA-256 implementation.

`random(count)` returns `Result<[]u8, string>`, with at most 1 MiB per call.
`decode_hex(value)` returns `Result<[]u8, string>`.
`argon2id_hash(password)` returns `Result<string, string>` containing
a PHC string with time cost 2, memory cost 19,456 KiB, one lane, a random
16-byte salt and a 32-byte hash. `argon2id_verify(password, phc)` accepts
Argon2id version 19 and bounds memory, iterations and parallelism before
calling the library. Password lengths are explicit, including embedded NUL.

`hmac_sha256(&key, &data)` returns 32 bytes.
`hkdf_sha256(&key, &salt, &info, length)` implements RFC 5869 and returns
`Result<[]u8, string>` with at most 8,160 bytes. Empty salt has the RFC-defined zero-key behavior.
`equal(&left, &right)` compares equal-length bytes without data-dependent
early exits. Lengths remain observable.

`aes256_gcm_seal(&key, &nonce, &aad, &plaintext)` returns
`Result<Sealed, string>` (`Sealed { ciphertext: []u8, tag: []u8 }`). The key is 32 bytes, nonce
12 bytes, and tag 16 bytes. Generate a fresh random nonce for each
encryption under the same key. Plaintext/ciphertext is bounded to 16 MiB,
and associated data to 1 MiB. `aes256_gcm_open` takes those same inputs
plus `&tag` and returns `Result<[]u8, string>`. It returns `.Err(...)` with no plaintext when
authentication fails. Callers own nonce uniqueness and key management.

`ed25519_keygen()` generates a 32-byte private key seed and matching 32-byte
public key, returning `Result<Keypair, string>` (`Keypair { private_key: []u8, public_key: []u8 }`).
`ed25519_public_key(&private_key)` derives the 32-byte public key from a
32-byte seed (`Result<[]u8, string>`). `ed25519_sign(&private_key, &message)` produces a 64-byte
PureEdDSA signature for messages up to 16 MiB (`Result<[]u8, string>`), and
`ed25519_verify(&public_key, &message, &signature)` verifies a 64-byte
signature against a 32-byte public key.

For handling sensitive key material and passwords safely, `crypto.zeroize(&mut buf)`
wipes a `[]u8` buffer in place with a compiler barrier against dead-store
elimination (and `crypto.zeroize_and_clear(&mut buf)` wipes then clears the vector).
`SecretBytes` is a `@resource(dispose)` buffer (`empty`, `from_bytes`, `from_slice`,
`from_text`, `random`, `.len()`, `.is_empty()`, `.as_bytes()`, `.as_mut_bytes()`,
`.equal(&other)` in constant time, `.clear()`) that automatically zeroizes its
bytes when dropped.

Libraries are loaded dynamically via `std.dl`: override paths explicitly with
`DUNDER_CRYPTO_LIB`, `OPENSSL_LIB_DIR`, or `DUNDER_ARGON2_LIB`, or rely on standard
system/Homebrew library names (`libcrypto.3.dylib`/`libcrypto.so.3` and
`libargon2.1.dylib`/`libargon2.so.1`). No compiler, subprocess, or application shim
implements the cryptographic operations.

Run `/absolute/path/to/dunder run tools/test_crypto.ddr -- /absolute/path/to/dunder`. Its native
fixture checks published NIST AES-256-GCM and RFC HMAC/HKDF vectors,
authentication failures, zero-length inputs, bounds, password verification,
random generation, zeroization, and `SecretBytes`. Additional runtimes can be selected with
`--runner TARGET=COMMAND`.
