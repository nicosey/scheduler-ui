# Cron Manager

A native desktop GUI for managing your crontab — view, add, toggle, delete, and run jobs, with a live run history.

## Features

- Reads your crontab on launch and writes changes back on every action
- Disabled jobs are preserved as commented lines (`# schedule command`) so they can be re-enabled
- Non-job lines (env vars, plain comments) are kept intact on write-back
- Run any job immediately with the ▶ button — output and duration are captured in the run history
- `--mock` flag for trying the app without touching your real crontab

## Requirements

- macOS (uses native Metal/OpenGL via `eframe`)
- Rust 1.70 or later — install from [rustup.rs](https://rustup.rs)

## Build & run

```sh
git clone https://github.com/yourname/cron-manager
cd cron-manager

# Run in development mode (reads your real crontab)
cargo run

# Run with mock data (no crontab access needed)
cargo run -- --mock

# Optimised build
cargo build --release
./target/release/cron-manager
```

## Usage

| Action | How |
| --- | --- |
| Expand a job | Click anywhere on the job row |
| Run a job now | Click ▶ on the right of any row |
| Enable / disable | Click ◉ / ○ — writes back to crontab immediately |
| Delete | Click ✕ — writes back to crontab immediately |
| Add a job | Click **+ New Job** in the top-right |
| Reload from crontab | Click **↻ Refresh** |

Cron expression format: `minute hour day-of-month month day-of-week`

```sh
# Every day at 07:00
0 7 * * *   /path/to/script.sh

# Every Sunday at midnight
0 0 * * 0   find /tmp -type f -mtime +7 -delete

# Every weekday at 08:30
30 8 * * 1-5  /usr/local/bin/my-job
```

## Release (macOS app bundle)

To produce a standalone `.app` you can distribute:

```sh
# 1. Build release binary
cargo build --release

# 2. Create the bundle structure
mkdir -p "Cron Manager.app/Contents/MacOS"
mkdir -p "Cron Manager.app/Contents/Resources"

# 3. Copy binary
cp target/release/cron-manager "Cron Manager.app/Contents/MacOS/cron-manager"

# 4. Write Info.plist
cat > "Cron Manager.app/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Cron Manager</string>
  <key>CFBundleExecutable</key>
  <string>cron-manager</string>
  <key>CFBundleIdentifier</key>
  <string>com.yourname.cron-manager</string>
  <key>CFBundleVersion</key>
  <string>0.1.0</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
EOF

# 5. Open it
open "Cron Manager.app"
```

To distribute, zip the `.app`:

```sh
zip -r "cron-manager-macos.zip" "Cron Manager.app"
```

## Project structure

```text
src/
  main.rs     — all app code (data models, crontab I/O, egui UI)
Cargo.toml
```

## Dependencies

| Crate | Purpose |
| --- | --- |
| `eframe` | Native window + immediate-mode GUI (egui) |
| `chrono` | Timestamps and duration formatting |
| `serde` + `serde_json` | Serialisation (run logs) |
