# Deploy

Ansible playbook that deploys `dotless` to a Debian/Ubuntu host behind Caddy.

## What it does

1. Installs build deps (`build-essential`, `pkg-config`, `libssl-dev`, …).
2. Creates a hardened `dotless` system user with no shell.
3. Installs `rustup` (stable, minimal) under that user — no global Rust.
4. Rsyncs the project source (excluding `target/`, `.git/`, `live_events.json`,
   `deploy/`) to `/opt/dotless/src/`.
5. Builds `cargo build --release --locked` as the `dotless` user.
6. Installs the binary to `/opt/dotless/dotless` and static assets to
   `/opt/dotless/static/`.
7. Seeds `/var/lib/dotless/live_events.json` from the local file **only if
   absent** — subsequent runs never clobber server state.
8. Renders a sandboxed `dotless.service` systemd unit and starts it.
9. Installs Caddy from the official Cloudsmith repo.
10. Renders a `Caddyfile` reverse-proxying `dotless.xyz` (and redirecting
    `www.dotless.xyz`) to `127.0.0.1:8080` with auto Let's Encrypt TLS.
11. Opens `80/tcp` and `443/tcp` in `ufw`.

The playbook is idempotent — re-run it to ship updates.

## Layout

```
deploy/
├── ansible.cfg
├── inventory.yml          # host + SSH key
├── requirements.yml       # ansible.posix, community.general
├── playbook.yml
└── templates/
    ├── Caddyfile.j2
    └── dotless.service.j2
```

On the server:

```
/opt/dotless/
├── dotless            # binary (root of WorkingDirectory)
├── static/            # served by axum ServeDir
├── src/               # build tree, kept for incremental builds
├── .cargo/  .rustup/  # toolchain, owned by dotless user
/etc/dotless/env       # optional override file (EnvironmentFile=-)
/etc/systemd/system/dotless.service
/etc/caddy/Caddyfile
/var/lib/dotless/live_events.json  # state (writable; ReadWritePaths in unit)
/var/log/caddy/access.log
```

## One-time setup (controller)

```bash
cd deploy
ansible-galaxy collection install -r requirements.yml
```

Make sure your SSH key is loaded: `ssh-add ~/.ssh/1und1`.

## Prerequisites on the host

- DNS A/AAAA records for **dotless.xyz** *and* **www.dotless.xyz** must point
  to `82.165.217.34` before first run, otherwise Caddy's ACME challenge will
  fail. Caddy will keep retrying once DNS is correct.
- The `vados` user with passwordless sudo (already set up).

## Deploy

```bash
# Dry-run first (recommended):
ansible-playbook playbook.yml --check --diff

# Real deploy:
ansible-playbook playbook.yml
```

First deploy takes ~5–10 minutes (rust toolchain download + full release build
of subxt et al.). Subsequent deploys are <1 minute thanks to incremental compile
and the on-server `target/` cache.

### Run a single stage

Each major section is tagged. Examples:

```bash
ansible-playbook playbook.yml --tags caddy        # re-render Caddyfile + reload
ansible-playbook playbook.yml --tags build        # rsync + cargo build + restart
ansible-playbook playbook.yml --tags service      # re-render systemd unit
ansible-playbook playbook.yml --tags rust         # rustup update
ansible-playbook playbook.yml --tags firewall     # ufw rules
```

## After deploy

```bash
ssh server systemctl status dotless caddy
ssh server journalctl -u dotless -f
```

Browse to https://dotless.xyz — Caddy should auto-issue a Let's Encrypt cert on
first hit.

## Overriding env vars

To override `ASSET_HUB_RPC`, `INGEST_REFRESH_SECS`, `RUST_LOG`, etc. without
editing the unit, drop a file at `/etc/dotless/env` on the server:

```
ASSET_HUB_RPC=wss://your-rpc.example
INGEST_REFRESH_SECS=120
RUST_LOG=dotless=debug
```

Then `systemctl restart dotless`.

## Notes / gotchas

- The rsync of source uses `sudo -n rsync` on the remote (the dest path is
  owned by the `dotless` user); the local `vados` account has passwordless
  sudo, so this works without prompting.
- `Cargo.lock` is committed; `--locked` guarantees the lockfile is honored.
- `live_events.json` is in `.gitignore` and excluded from the rsync. The
  ingestor on the server maintains its own copy in `/var/lib/dotless/`.
- The systemd unit pins everything except `/var/lib/dotless` as read-only and
  drops all capabilities. If you ever add a file write outside that directory,
  extend `ReadWritePaths=`.
