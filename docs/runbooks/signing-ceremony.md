# Root signing ceremony runbook (Model-2, one-shot)

Operational steps for the **maintainer** to mint the mnemra root Ed25519 keypair
and produce the signed `mnemra-echo` manifest. This is a **one-shot ceremony**:
the private root key is generated once, kept in maintainer custody, and never
enters the repository or any runtime-read directory. No agent or automation ever
touches the private key.

This runbook covers **producer round** work only (keygen + manifest signing).
Embedding the resulting public key into the binary and wiring the pin gate into
CI is the **round-2 build step**, summarised at the end.

> Producer tool: `cargo run -p mnemra-host --bin sign-ceremony` (wrapped by the
> `just sign-keygen` and `just sign-ceremony` recipes). Its unit tests prove the
> produced manifest verifies against the real runtime verifier
> (`signing::verify::verify_plugin`); run them with
> `cargo test -p mnemra-host --bin sign-ceremony`.

## Two failure modes to avoid (read first)

The signing chain round-trips only if BOTH of these hold. Both are enforced by
the tool, but they are the mistakes that silently break a hand-rolled ceremony:

1. **Ed25519-dalek end-to-end — no OpenSSL, no other crate.** The signature and
   the keypair are produced with the exact `ed25519-dalek` library the runtime
   verifies with. Signing with `openssl`, a different Ed25519 implementation, or
   a PEM/DER-wrapped key will produce bytes that do not match what
   `verify_plugin` reconstructs, and the plugin will be rejected at load. The
   private key file is the **32-byte raw Ed25519 seed** produced by
   `just sign-keygen` — nothing else.

2. **Hash the committed `.wasm`, not a `target/` rebuild.** The `[component].hash`
   must be BLAKE3 over the **exact bytes of the committed, signed `.wasm`** that
   the runtime loads — not the output of a fresh `cargo build`. Two builds of the
   same source can differ byte-for-byte; hashing a rebuild yields a hash that
   will not match the loaded artifact, and the content-hash gate will reject the
   plugin as tampered. Always point `sign-ceremony sign` at the committed
   artifact path.

## Step 1 — Generate the root keypair

```sh
just sign-keygen <key-path>
```

- Generates a fresh Ed25519 keypair from the OS CSPRNG.
- Writes the **32-byte private seed** to `<key-path>` with mode `600`.
- Prints the **public key (hex)** to stdout. Record it — it becomes the embedded
  root in round-2.

The command refuses to overwrite an existing file, so `<key-path>` must be new.

## Step 2 — Put the private key in custody, mode 600, OUTSIDE every runtime-read dir

The host's startup file-mode check requires the admin-token and
signing-verification files to be mode `600`. The **private root key** is more
sensitive still: it must live **mode `600` and OUTSIDE all three directories the
runtime reads**, so that no load path, log, or backup of a runtime directory can
ever surface it:

1. the plugin/manifest directory (`plugins/mnemra-echo/`),
2. the signed-artifact directory (wherever the committed `.wasm` is loaded from),
3. the admin-token file's directory.

**Recommended custody location — outside the repository entirely:**

```sh
mkdir -p ~/.config/mnemra
chmod 700 ~/.config/mnemra
just sign-keygen ~/.config/mnemra/root-signing.key   # already written mode 600
```

If you must keep it inside the working tree, use a dedicated `.secrets/`
directory that is **gitignored** and is none of the three runtime-read
directories above — and confirm the `.gitignore` entry before generating the
key. Outside-the-repo custody is strongly preferred.

Verify custody after generation:

```sh
stat -f "%Sp" ~/.config/mnemra/root-signing.key   # must print -rw-------
```

## Step 3 — Sign the manifest

```sh
just sign-ceremony <key-path> <wasm-path> <manifest-path>
```

Where:

- `<key-path>` — the custody private key from Step 2 (read **in place**; the tool
  never copies it out),
- `<wasm-path>` — the **committed** signed `.wasm` artifact the runtime loads
  (see failure mode 2),
- `<manifest-path>` — `plugins/mnemra-echo/manifest.toml`.

The tool then:

1. reads the private key in place,
2. computes BLAKE3 over the exact `<wasm-path>` bytes,
3. embeds a `[component]` block (`hash_alg = "blake3"`, `hash = <blake3-hex>`)
   **inside the signed body** (above `[signature]`, so it is authenticated),
4. signs the signed payload (the manifest body up to `\n[signature]`),
5. writes the manifest back with a populated `[signature]`
   (`algorithm`, `public_key`, `sig_bytes`, `signed_at`),
6. **self-verifies the result against `verify_plugin` before writing** — if the
   round-trip does not hold (e.g. the manifest is not `core = true`), nothing is
   written and the tool exits non-zero,
7. prints the **public key (hex)** to stdout (identical to Step 1's output).

Re-running the command is idempotent: it strips the prior `[signature]` and
`[component]` and re-emits exactly one of each.

## Step 4 — Hand off to the round-2 build step

Provide three outputs to the round-2 build/integration step:

1. **the public key (hex)** from Step 1/3 — set BOTH the embedded root literal
   (`signing::root_material::ROOT`) AND the independently-declared pin
   (`signing::root_material::ROOT_PIN`) to this value. They are two separate
   literals holding the same bytes: the `verify-signing-root` gate passes only
   when they byte-match, so a later root swap without a matching pin update
   fails the gate by design;
2. **the re-signed `plugins/mnemra-echo/manifest.toml`**;
3. **the committed signed `.wasm`** (the exact bytes hashed in Step 3).

Round-2 then embeds the root, un-ignores the live pin test, wires
`verify-signing-root` into `just ci`, and commits. The private key stays in
maintainer custody and is **never** committed.
