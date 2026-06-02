# Deployment (systemd)

`betting-api` runs as a long-running systemd service (this replaces the old PM2
`ecosystem.config.json`).

```bash
cargo build --release                       # builds target/release/betting_api
sudo cp deploy/betting-api.service /etc/systemd/system/
# edit WorkingDirectory / EnvironmentFile / ExecStart paths to match your host
sudo systemctl daemon-reload
sudo systemctl enable --now betting-api
sudo systemctl status betting-api
journalctl -u betting-api -f                 # logs (replaces the PM2 log files)
```

Config (DB path, Rust API URL, etc.) comes from the `EnvironmentFile` (`.env`),
never committed.
