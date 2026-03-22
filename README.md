# Scheduler UI

A native macOS desktop GUI for managing scheduled jobs — supports both **cron** and **launchd**. View, add, toggle, delete, and run jobs, with a live run history.

## Features

- Reads your crontab and `~/Library/LaunchAgents/` on launch; writes changes back immediately
- Cron: disabled jobs are preserved as commented lines (`# schedule command`) so they can be re-enabled
- Launchd: toggle loads/unloads via `launchctl`; delete removes the plist file
- Source badges on every row — **CRON** or **LAUNCHD** — so you always know what's driving a job
- Add new jobs to either backend from the same dialog
- Run any job immediately with the ▶ button — output and duration are captured in the run history
- `--mock` flag for trying the app without touching your real crontab or launchd agents

## Requirements

- macOS (uses native Metal/OpenGL via `eframe`)
- Rust 1.70 or later — install from [rustup.rs](https://rustup.rs)

## Build & run

```sh
git clone https://github.com/nicosey/cron-manager
cd cron-manager

# Run in development mode (reads your real crontab + LaunchAgents)
cargo run

# Run with mock data (no system access needed)
cargo run -- --mock

# Optimised build
cargo build --release
./target/release/scheduler-ui
```

## Usage

| Action | How |
| --- | --- |
| Expand a job | Click anywhere on the job row |
| Run a job now | Click ▶ on the right of any row |
| Enable / disable | Click ◉ / ○ — writes back immediately |
| Delete | Click ✕ — writes back immediately |
| Add a job | Click **+ New Job**, choose Cron or Launchd |
| Reload | Click **↻ Refresh** |

### Adding a cron job

Choose **Cron** in the source selector. Schedule uses standard cron expression format:
`minute hour day-of-month month day-of-week`

```sh
# Every day at 07:00
0 7 * * *   /path/to/script.sh

# Every Sunday at midnight
0 0 * * 0   find /tmp -type f -mtime +7 -delete

# Every weekday at 08:30
30 8 * * 1-5  /usr/local/bin/my-job
```

### Adding a launchd agent

Choose **Launchd** in the source selector. Enter a reverse-DNS label (e.g. `com.user.my-task`) — this becomes the plist filename in `~/Library/LaunchAgents/`. The same cron schedule format is used and converted to `StartCalendarInterval` automatically.

## Release (macOS app bundle)

```sh
# 1. Build release binary
cargo build --release

# 2. Create the bundle structure
mkdir -p "Scheduler UI.app/Contents/MacOS"
mkdir -p "Scheduler UI.app/Contents/Resources"

# 3. Copy binary
cp target/release/scheduler-ui "Scheduler UI.app/Contents/MacOS/scheduler-ui"

# 4. Write Info.plist
cat > "Scheduler UI.app/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>Scheduler UI</string>
  <key>CFBundleExecutable</key>
  <string>scheduler-ui</string>
  <key>CFBundleIdentifier</key>
  <string>com.yourname.scheduler-ui</string>
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
open "Scheduler UI.app"
```

To distribute, zip the `.app`:

```sh
zip -r "scheduler-ui-macos.zip" "Scheduler UI.app"
```

## Project structure

```text
src/
  main.rs     — all app code (data models, cron/launchd I/O, egui UI)
Cargo.toml
```

## Dependencies

| Crate | Purpose |
| --- | --- |
| `eframe` | Native window + immediate-mode GUI (egui) |
| `chrono` | Timestamps and duration formatting |
| `plist` | Parsing `~/Library/LaunchAgents/*.plist` files |
| `serde` + `serde_json` | Serialisation (run logs) |
