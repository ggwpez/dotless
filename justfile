set dotenv-load

# Port the server listens on
port := "8080"

# === Local dev ===

# Run the server (listens on :{{port}})
run:
    PORT={{port}} cargo run --release

# Run with debug logging (listens on :{{port}})
debug:
    PORT={{port}} RUST_LOG=dotless=debug cargo run --release

# Check compilation
check:
    cargo check

# Build release binary
build:
    cargo build --release

# === Deploy (Ansible → 82.165.217.34) ===

# One-time: install required ansible collections
deploy-setup:
    cd deploy && ansible-galaxy collection install -r requirements.yml

# Dry-run: show what would change, no writes
deploy-dry:
    cd deploy && ansible-playbook playbook.yml --check --diff

# Full deploy
deploy:
    cd deploy && ansible-playbook playbook.yml

# Re-run only the build stage (rsync + cargo build + restart)
deploy-build:
    cd deploy && ansible-playbook playbook.yml --tags build

# Re-render Caddyfile and reload Caddy
deploy-caddy:
    cd deploy && ansible-playbook playbook.yml --tags caddy

# Re-render systemd unit and restart dotless
deploy-service:
    cd deploy && ansible-playbook playbook.yml --tags service

# Update Rust toolchain on the server
deploy-rust:
    cd deploy && ansible-playbook playbook.yml --tags rust

# Open ufw 80/443
deploy-firewall:
    cd deploy && ansible-playbook playbook.yml --tags firewall

# Pass-through: `just deploy-ansible -- --tags caddy --check`
deploy-ansible *ARGS:
    cd deploy && ansible-playbook playbook.yml {{ARGS}}

# === Server ops ===

# Tail dotless logs
logs:
    ssh server journalctl -u dotless -f

# Tail caddy logs (access + errors)
logs-caddy:
    ssh server sudo journalctl -u caddy -f

# Verify the app is reachable end-to-end (localhost → caddy → public IP)
verify:
    @echo "--- dotless binary on server ---"
    ssh server ls -la /opt/dotless/dotless
    @echo "--- dotless service age ---"
    ssh server systemctl show dotless -p ActiveEnterTimestamp,MainPID,SubState
    @echo "--- version: app on localhost:8080 (direct) ---"
    ssh server "curl -sS http://localhost:8080/ | grep -o 'Source Code (v[0-9])'"
    @echo "--- version: caddy on localhost:443 (caddy → app) ---"
    ssh server "curl -sSk --resolve dotless.xyz:443:127.0.0.1 https://dotless.xyz/ | grep -o 'Source Code (v[0-9])'"
    @echo "--- version: HTTPS via DNS (controller → public) ---"
    curl -sS https://dotless.xyz/ | grep -o 'Source Code (v[0-9])'

# Service status (dotless + caddy)
status:
    ssh server "systemctl status dotless caddy --no-pager"

# Restart dotless without redeploy
restart:
    ssh server sudo systemctl restart dotless

# Restart Caddy (clears ACME backoff so it retries cert issuance)
restart-caddy:
    ssh server sudo systemctl restart caddy

# SSH in
shell:
    ssh server

# Diagnose caddy issues (perms, recent logs, config validation)
caddy-diag:
    @echo "=== Caddy data tree (deep) ==="
    ssh server "sudo find /var/lib/caddy /etc/caddy /root -type f 2>/dev/null | grep -iE 'cert|\\.crt|\\.key|\\.pem' | head -20"
    @echo ""
    @echo "=== Cert from server perspective (apex) ==="
    ssh server "echo Q | openssl s_client -servername dotless.xyz -connect 127.0.0.1:443 2>/dev/null | openssl x509 -noout -issuer -subject -dates -ext subjectAltName 2>/dev/null"
    @echo ""
    @echo "=== Cert from server perspective (www) ==="
    ssh server "echo Q | openssl s_client -servername www.dotless.xyz -connect 127.0.0.1:443 2>/dev/null | openssl x509 -noout -issuer -subject -dates -ext subjectAltName 2>/dev/null"
