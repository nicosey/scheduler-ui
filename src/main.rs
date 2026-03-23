use eframe::egui;
use chrono::{DateTime, Utc, Local, Duration};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};

// ── Data models ──────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum JobSource {
    Cron,
    Launchd { label: String, plist_path: String },
}

#[derive(Clone, Serialize, Deserialize)]
struct CronJob {
    id: usize,
    name: String,
    schedule: String,
    command: String,
    enabled: bool,
    source: JobSource,
    last_run: Option<DateTime<Utc>>,
    last_status: Option<RunStatus>,
    last_duration: Option<String>,
    logs: Vec<RunLog>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
enum RunStatus {
    Success,
    Error,
    Running,
}

#[derive(Clone, Serialize, Deserialize)]
struct RunLog {
    timestamp: DateTime<Utc>,
    status: RunStatus,
    duration: String,
    output: String,
}

// ── Crontab parsing / writing ─────────────────────────────────────────────────

fn parse_crontab() -> (Vec<CronJob>, Vec<String>) {
    let output = Command::new("crontab").arg("-l").output();
    let mut jobs = Vec::new();
    let mut preamble = Vec::new();

    if let Ok(out) = output {
        let content = String::from_utf8_lossy(&out.stdout);
        let mut id = 0;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                preamble.push(line.to_string());
                continue;
            }
            if trimmed.starts_with('#') {
                let rest = trimmed[1..].trim();
                if let Some(job) = try_parse_job(id, rest, false, JobSource::Cron) {
                    jobs.push(job);
                    id += 1;
                    continue;
                }
                preamble.push(line.to_string());
                continue;
            }
            if let Some(job) = try_parse_job(id, trimmed, true, JobSource::Cron) {
                jobs.push(job);
                id += 1;
            } else {
                preamble.push(line.to_string());
            }
        }
    }

    (jobs, preamble)
}

fn try_parse_job(id: usize, line: &str, enabled: bool, source: JobSource) -> Option<CronJob> {
    let parts: Vec<&str> = line.splitn(6, char::is_whitespace).collect();
    if parts.len() >= 6 {
        let schedule = format!("{} {} {} {} {}",
            parts[0], parts[1], parts[2], parts[3], parts[4]);
        let command = parts[5].to_string();
        let name = extract_name(&command);
        Some(CronJob {
            id,
            name,
            schedule,
            command,
            enabled,
            source,
            last_run: None,
            last_status: None,
            last_duration: None,
            logs: Vec::new(),
        })
    } else {
        None
    }
}

fn write_crontab(preamble: &[String], jobs: &[CronJob]) -> Result<(), String> {
    let mut lines: Vec<String> = preamble.to_vec();
    for job in jobs {
        let entry = format!("{} {}", job.schedule, job.command);
        lines.push(if job.enabled { entry } else { format!("# {}", entry) });
    }
    let content = lines.join("\n") + "\n";

    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn crontab: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write crontab: {e}"))?;
    }

    let status = child.wait().map_err(|e| format!("crontab wait failed: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("crontab exited with non-zero status".into())
    }
}

// ── Launchd backend ───────────────────────────────────────────────────────────

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(std::path::PathBuf::from)
}

fn is_launchd_loaded(label: &str) -> bool {
    Command::new("launchctl")
        .args(["list", label])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn launchctl_load(plist_path: &str) -> Result<(), String> {
    let out = Command::new("launchctl")
        .args(["load", plist_path])
        .output()
        .map_err(|e| format!("launchctl load failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn launchctl_unload(plist_path: &str) -> Result<(), String> {
    let out = Command::new("launchctl")
        .args(["unload", plist_path])
        .output()
        .map_err(|e| format!("launchctl unload failed: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

fn delete_launchd_job(plist_path: &str) -> Result<(), String> {
    let _ = launchctl_unload(plist_path);
    std::fs::remove_file(plist_path)
        .map_err(|e| format!("Cannot remove plist: {e}"))
}

fn sci_dict_to_cron(d: &plist::Dictionary) -> String {
    let min  = d.get("Minute") .and_then(|v| v.as_signed_integer()).map(|n| n.to_string()).unwrap_or_else(|| "*".into());
    let hour = d.get("Hour")   .and_then(|v| v.as_signed_integer()).map(|n| n.to_string()).unwrap_or_else(|| "*".into());
    let dom  = d.get("Day")    .and_then(|v| v.as_signed_integer()).map(|n| n.to_string()).unwrap_or_else(|| "*".into());
    let mon  = d.get("Month")  .and_then(|v| v.as_signed_integer()).map(|n| n.to_string()).unwrap_or_else(|| "*".into());
    let dow  = d.get("Weekday").and_then(|v| v.as_signed_integer()).map(|n| n.to_string()).unwrap_or_else(|| "*".into());
    format!("{} {} {} {} {}", min, hour, dom, mon, dow)
}

fn parse_start_calendar_interval(dict: &plist::Dictionary) -> String {
    match dict.get("StartCalendarInterval") {
        Some(v) => {
            if let Some(d) = v.as_dictionary() {
                return sci_dict_to_cron(d);
            }
            if let Some(arr) = v.as_array() {
                if let Some(first) = arr.first().and_then(|v| v.as_dictionary()) {
                    return sci_dict_to_cron(first);
                }
            }
            "* * * * *".into()
        }
        None => {
            if dict.get("RunAtLoad").and_then(|v| v.as_boolean()) == Some(true) {
                return "At load".into();
            }
            "* * * * *".into()
        }
    }
}

fn parse_launchd_agents(id_offset: usize) -> Vec<CronJob> {
    let mut jobs = Vec::new();
    let agents_dir = match home_dir() {
        Some(h) => h.join("Library/LaunchAgents"),
        None => return jobs,
    };

    let entries = match std::fs::read_dir(&agents_dir) {
        Ok(e) => e,
        Err(_) => return jobs,
    };

    let mut id = id_offset;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }
        let path_str = path.to_string_lossy().to_string();

        let value = match plist::Value::from_file(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let dict = match value.as_dictionary() {
            Some(d) => d,
            None => continue,
        };

        let label = match dict.get("Label").and_then(|v| v.as_string()) {
            Some(l) => l.to_string(),
            None => continue,
        };

        let command = dict.get("ProgramArguments")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_string()).collect::<Vec<_>>().join(" "))
            .or_else(|| dict.get("Program").and_then(|v| v.as_string()).map(|s| s.to_string()))
            .unwrap_or_else(|| label.clone());

        let schedule = parse_start_calendar_interval(dict);
        let enabled = is_launchd_loaded(&label);
        let name = label.split('.').last().unwrap_or(&label).replace('-', "_");
        let name = title_case(&name);

        jobs.push(CronJob {
            id,
            name,
            schedule,
            command,
            enabled,
            source: JobSource::Launchd { label, plist_path: path_str },
            last_run: None,
            last_status: None,
            last_duration: None,
            logs: Vec::new(),
        });
        id += 1;
    }

    jobs
}

fn write_launchd_plist(label: &str, command: &str, schedule: &str) -> Result<String, String> {
    let agents_dir = home_dir()
        .ok_or("Cannot determine HOME")?
        .join("Library/LaunchAgents");
    std::fs::create_dir_all(&agents_dir)
        .map_err(|e| format!("Cannot create LaunchAgents dir: {e}"))?;

    let path = agents_dir.join(format!("{}.plist", label));
    let path_str = path.to_string_lossy().to_string();

    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() != 5 {
        return Err(format!("Invalid schedule: {}", schedule));
    }
    let (min, hour, dom, mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);

    let mut sci = String::from("<key>StartCalendarInterval</key>\n    <dict>\n");
    if min  != "*" { sci.push_str(&format!("        <key>Minute</key><integer>{min}</integer>\n")); }
    if hour != "*" { sci.push_str(&format!("        <key>Hour</key><integer>{hour}</integer>\n")); }
    if dom  != "*" { sci.push_str(&format!("        <key>Day</key><integer>{dom}</integer>\n")); }
    if mon  != "*" { sci.push_str(&format!("        <key>Month</key><integer>{mon}</integer>\n")); }
    if dow  != "*" { sci.push_str(&format!("        <key>Weekday</key><integer>{dow}</integer>\n")); }
    sci.push_str("    </dict>");

    let xml = format!(
r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>/bin/sh</string>
        <string>-c</string>
        <string>{command}</string>
    </array>
    {sci}
</dict>
</plist>"#
    );

    std::fs::write(&path, xml)
        .map_err(|e| format!("Cannot write plist: {e}"))?;

    launchctl_load(&path_str)?;
    Ok(path_str)
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn extract_name(command: &str) -> String {
    let parts: Vec<&str> = command.split_whitespace().collect();
    for part in &parts {
        if part.contains(".py") || part.contains(".sh") || part.contains(".rb") {
            if let Some(name) = part.rsplit('/').next() {
                let name = name.replace(".py", "").replace(".sh", "").replace(".rb", "");
                return title_case(&name);
            }
        }
    }
    parts.first()
        .map(|s| s.rsplit('/').next().unwrap_or(s).to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

fn title_case(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sample_jobs() -> Vec<CronJob> {
    let now = Utc::now();
    vec![
        CronJob {
            id: 0,
            name: "Robotics Briefing".into(),
            schedule: "0 7 * * *".into(),
            command: "cd /Users/m4server/projects/briefing && python3 briefing.py --config config/robotics.json".into(),
            enabled: true,
            source: JobSource::Cron,
            last_run: Some(now - Duration::hours(3)),
            last_status: Some(RunStatus::Success),
            last_duration: Some("14s".into()),
            logs: vec![
                RunLog { timestamp: now - Duration::hours(3),  status: RunStatus::Success, duration: "14s".into(), output: "Briefing sent to Telegram successfully.".into() },
                RunLog { timestamp: now - Duration::hours(27), status: RunStatus::Success, duration: "12s".into(), output: "Briefing sent to Telegram successfully.".into() },
                RunLog { timestamp: now - Duration::hours(51), status: RunStatus::Error,   duration: "3s".into(),  output: "Set TELEGRAM_BOT_TOKEN in .env".into() },
                RunLog { timestamp: now - Duration::hours(75), status: RunStatus::Success, duration: "15s".into(), output: "Briefing sent to Telegram successfully.".into() },
            ],
        },
        CronJob {
            id: 1,
            name: "Backup Database".into(),
            schedule: "30 2 * * *".into(),
            command: "pg_dump mydb | gzip > /backups/mydb_$(date +%Y%m%d).sql.gz".into(),
            enabled: true,
            source: JobSource::Cron,
            last_run: Some(now - Duration::hours(8)),
            last_status: Some(RunStatus::Success),
            last_duration: Some("45s".into()),
            logs: vec![
                RunLog { timestamp: now - Duration::hours(8),  status: RunStatus::Success, duration: "45s".into(), output: "Backup completed: mydb_20260322.sql.gz (234MB)".into() },
                RunLog { timestamp: now - Duration::hours(32), status: RunStatus::Success, duration: "42s".into(), output: "Backup completed: mydb_20260321.sql.gz (231MB)".into() },
            ],
        },
        CronJob {
            id: 2,
            name: "Clear Temp Files".into(),
            schedule: "0 0 * * 0".into(),
            command: "find /tmp -type f -mtime +7 -delete".into(),
            enabled: false,
            source: JobSource::Cron,
            last_run: Some(now - Duration::days(6)),
            last_status: Some(RunStatus::Success),
            last_duration: Some("2s".into()),
            logs: vec![
                RunLog { timestamp: now - Duration::days(6), status: RunStatus::Success, duration: "2s".into(), output: "Removed 47 files, freed 1.2GB".into() },
            ],
        },
        CronJob {
            id: 3,
            name: "Backup Photos".into(),
            schedule: "0 2 * * *".into(),
            command: "/usr/local/bin/rsync -a ~/Pictures /Volumes/Backup/Pictures".into(),
            enabled: true,
            source: JobSource::Launchd {
                label: "com.user.backup-photos".into(),
                plist_path: "/Users/user/Library/LaunchAgents/com.user.backup-photos.plist".into(),
            },
            last_run: Some(now - Duration::hours(14)),
            last_status: Some(RunStatus::Success),
            last_duration: Some("28s".into()),
            logs: vec![
                RunLog { timestamp: now - Duration::hours(14), status: RunStatus::Success, duration: "28s".into(), output: "Photos synced successfully.".into() },
            ],
        },
        CronJob {
            id: 4,
            name: "Log Cleanup".into(),
            schedule: "0 0 * * 0".into(),
            command: "/bin/sh -c 'find ~/Library/Logs -name \"*.log\" -mtime +30 -delete'".into(),
            enabled: false,
            source: JobSource::Launchd {
                label: "com.user.log-cleanup".into(),
                plist_path: "/Users/user/Library/LaunchAgents/com.user.log-cleanup.plist".into(),
            },
            last_run: None,
            last_status: None,
            last_duration: None,
            logs: Vec::new(),
        },
    ]
}

// ── Human-readable helpers ────────────────────────────────────────────────────

fn human_schedule(expr: &str) -> String {
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() != 5 {
        return expr.to_string();
    }
    let (min, hour, dom, _mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);
    let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

    if dom == "*" && dow == "*" {
        return format!("Daily at {:0>2}:{:0>2}", hour, min);
    }
    if dom == "*" && dow != "*" {
        let days: String = dow.split(',')
            .filter_map(|d| d.parse::<usize>().ok().and_then(|i| day_names.get(i)))
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        return format!("{} at {:0>2}:{:0>2}", days, hour, min);
    }
    expr.to_string()
}

fn time_ago(dt: &DateTime<Utc>) -> String {
    let diff = Utc::now().signed_duration_since(*dt);
    let mins = diff.num_minutes();
    if mins < 1  { return "just now".into(); }
    if mins < 60 { return format!("{}m ago", mins); }
    let hrs = diff.num_hours();
    if hrs < 24  { return format!("{}h ago", hrs); }
    format!("{}d ago", diff.num_days())
}

// ── Colors ────────────────────────────────────────────────────────────────────

struct Theme;
impl Theme {
    const BG: egui::Color32          = egui::Color32::from_rgb(15,  16,  23);
    const PANEL: egui::Color32       = egui::Color32::from_rgb(26,  27,  35);
    const BORDER: egui::Color32      = egui::Color32::from_rgb(42,  43,  53);
    const TEXT: egui::Color32        = egui::Color32::from_rgb(226, 232, 240);
    const TEXT_DIM: egui::Color32    = egui::Color32::from_rgb(100, 116, 139);
    const TEXT_MUTED: egui::Color32  = egui::Color32::from_rgb(148, 163, 184);
    const ACCENT: egui::Color32      = egui::Color32::from_rgb(99,  102, 241);
    const SUCCESS: egui::Color32     = egui::Color32::from_rgb(52,  211, 153);
    const ERROR: egui::Color32       = egui::Color32::from_rgb(248, 113, 113);
    const SELECTED: egui::Color32    = egui::Color32::from_rgb(30,  31,  42);
    const LAUNCHD: egui::Color32     = egui::Color32::from_rgb(168, 85,  247);
}

// ── App state ─────────────────────────────────────────────────────────────────

struct SchedulerApp {
    jobs: Vec<CronJob>,
    crontab_preamble: Vec<String>,
    use_mock: bool,
    selected: Option<usize>,
    show_add_dialog: bool,
    new_name: String,
    new_schedule: String,
    new_command: String,
    new_label: String,
    new_source_is_launchd: bool,
    status_msg: Option<String>,
}

impl SchedulerApp {
    fn new(jobs: Vec<CronJob>, crontab_preamble: Vec<String>, use_mock: bool) -> Self {
        Self {
            jobs,
            crontab_preamble,
            use_mock,
            selected: None,
            show_add_dialog: false,
            new_name: String::new(),
            new_schedule: String::new(),
            new_command: String::new(),
            new_label: String::new(),
            new_source_is_launchd: false,
            status_msg: None,
        }
    }

    fn save_crontab(&mut self) {
        if self.use_mock { return; }
        let cron_jobs: Vec<CronJob> = self.jobs.iter()
            .filter(|j| j.source == JobSource::Cron)
            .cloned()
            .collect();
        match write_crontab(&self.crontab_preamble, &cron_jobs) {
            Ok(_)  => self.status_msg = Some("Crontab saved.".into()),
            Err(e) => self.status_msg = Some(format!("Error: {e}")),
        }
    }

    fn toggle_launchd(&mut self, id: usize) {
        let Some(job) = self.jobs.iter().find(|j| j.id == id) else { return };
        let JobSource::Launchd { plist_path, .. } = job.source.clone() else { return };
        let currently_enabled = job.enabled;

        let result = if currently_enabled {
            launchctl_unload(&plist_path).map(|_| false)
        } else {
            launchctl_load(&plist_path).map(|_| true)
        };

        match result {
            Ok(new_state) => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                    job.enabled = new_state;
                }
                self.status_msg = Some(if new_state { "Job loaded." } else { "Job unloaded." }.into());
            }
            Err(e) => self.status_msg = Some(format!("Error: {e}")),
        }
    }

    fn clear_form(&mut self) {
        self.new_name.clear();
        self.new_schedule.clear();
        self.new_command.clear();
        self.new_label.clear();
        self.new_source_is_launchd = false;
    }
}

impl eframe::App for SchedulerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut style = (*ctx.style()).clone();
        style.visuals.override_text_color = Some(Theme::TEXT);
        style.visuals.panel_fill = Theme::BG;
        style.visuals.window_fill = Theme::PANEL;
        style.visuals.widgets.noninteractive.bg_fill = Theme::PANEL;
        style.visuals.widgets.inactive.bg_fill = Theme::PANEL;
        style.visuals.widgets.inactive.weak_bg_fill = Theme::PANEL;
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        ctx.set_style(style);

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Theme::BG).inner_margin(egui::Margin::same(20)))
            .show(ctx, |ui| {
                self.render_header(ui);
                ui.add_space(12.0);
                self.render_stats(ui);
                ui.add_space(12.0);
                self.render_job_list(ui);
                ui.add_space(8.0);

                ui.vertical_centered(|ui| {
                    if let Some(msg) = &self.status_msg {
                        let color = if msg.starts_with("Error") { Theme::ERROR } else { Theme::SUCCESS };
                        ui.label(egui::RichText::new(msg).size(11.0).color(color));
                    } else {
                        let footer = if self.use_mock {
                            "Mock mode · changes not saved"
                        } else {
                            "Powered by cron + launchd · Click a job to expand"
                        };
                        ui.label(egui::RichText::new(footer).size(10.0).color(Theme::TEXT_DIM));
                    }
                });
            });

        if self.show_add_dialog {
            self.render_add_dialog(ctx);
        }
    }
}

impl SchedulerApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        let active = self.jobs.iter().filter(|j| j.enabled).count();

        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 8.0, Theme::ACCENT);
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "⏱",
                egui::FontId::proportional(18.0), Theme::TEXT);

            ui.vertical(|ui| {
                let title = if self.use_mock { "Scheduler UI (mock)" } else { "Scheduler UI" };
                ui.label(egui::RichText::new(title).size(20.0).strong()
                    .color(egui::Color32::from_rgb(241, 245, 249)));
                ui.label(egui::RichText::new(format!("{} jobs · {} active", self.jobs.len(), active))
                    .size(11.0).color(Theme::TEXT_DIM));
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new("+ New Job").size(12.0).color(egui::Color32::WHITE))
                    .fill(Theme::ACCENT).corner_radius(8.0).min_size(egui::vec2(90.0, 32.0))
                ).clicked() {
                    self.show_add_dialog = true;
                }

                if ui.add(egui::Button::new(egui::RichText::new("↻ Refresh").size(12.0).color(Theme::TEXT_MUTED))
                    .fill(Theme::PANEL).stroke(egui::Stroke::new(1.0, Theme::BORDER))
                    .corner_radius(8.0).min_size(egui::vec2(80.0, 32.0))
                ).clicked() {
                    if self.use_mock {
                        self.jobs = sample_jobs();
                    } else {
                        let (mut jobs, preamble) = parse_crontab();
                        let launchd = parse_launchd_agents(jobs.len());
                        jobs.extend(launchd);
                        self.jobs = if jobs.is_empty() { sample_jobs() } else { jobs };
                        self.crontab_preamble = preamble;
                    }
                    self.status_msg = None;
                }
            });
        });
    }

    fn render_stats(&self, ui: &mut egui::Ui) {
        let active  = self.jobs.iter().filter(|j| j.enabled).count();
        let success = self.jobs.iter().filter(|j| j.last_status == Some(RunStatus::Success)).count();
        let errors  = self.jobs.iter().filter(|j| j.last_status == Some(RunStatus::Error)).count();

        ui.columns(3, |cols| {
            for (i, (label, value, color)) in [
                ("ACTIVE",  active,  Theme::ACCENT),
                ("PASSING", success, Theme::SUCCESS),
                ("FAILING", errors,  Theme::ERROR),
            ].iter().enumerate() {
                egui::Frame::new()
                    .fill(Theme::PANEL)
                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(&mut cols[i], |ui| {
                        ui.label(egui::RichText::new(*label).size(10.0).color(Theme::TEXT_DIM));
                        ui.label(egui::RichText::new(format!("{}", value))
                            .size(26.0).strong().color(*color));
                    });
            }
        });
    }

    fn render_job_list(&mut self, ui: &mut egui::Ui) {
        let mut toggle_id = None;
        let mut run_id    = None;
        let mut delete_id = None;
        let mut click_id  = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for job in &self.jobs {
                let is_selected  = self.selected == Some(job.id);
                let bg           = if is_selected { Theme::SELECTED } else { Theme::PANEL };
                let stroke_color = if is_selected { Theme::ACCENT } else { Theme::BORDER };

                let response = egui::Frame::new()
                    .fill(bg)
                    .stroke(egui::Stroke::new(1.0, stroke_color))
                    .corner_radius(10.0)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let status_color = if !job.enabled {
                                Theme::TEXT_DIM
                            } else {
                                match &job.last_status {
                                    Some(RunStatus::Success) => Theme::SUCCESS,
                                    Some(RunStatus::Error)   => Theme::ERROR,
                                    Some(RunStatus::Running) => Theme::ACCENT,
                                    None => Theme::TEXT_DIM,
                                }
                            };
                            let (dot_rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter().circle_filled(dot_rect.center(), 4.0, status_color);

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&job.name)
                                        .size(14.0).strong().color(egui::Color32::from_rgb(241, 245, 249)));
                                    if !job.enabled {
                                        ui.label(egui::RichText::new("PAUSED").size(9.0).color(Theme::TEXT_DIM));
                                    }
                                    let (badge, badge_color) = match &job.source {
                                        JobSource::Cron      => ("CRON",     Theme::TEXT_DIM),
                                        JobSource::Launchd { .. } => ("LAUNCHD", Theme::LAUNCHD),
                                    };
                                    ui.label(egui::RichText::new(badge).size(9.0).color(badge_color));
                                });
                                ui.label(egui::RichText::new(human_schedule(&job.schedule))
                                    .size(11.0).color(Theme::TEXT_DIM));
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new(egui::RichText::new("✕").size(12.0).color(Theme::TEXT_DIM))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text("Delete job").clicked() {
                                    delete_id = Some(job.id);
                                }

                                let (toggle_text, toggle_color, toggle_tip) = if job.enabled {
                                    ("◉", Theme::SUCCESS, "Disable job")
                                } else {
                                    ("○", Theme::TEXT_DIM, "Enable job")
                                };
                                if ui.add(egui::Button::new(egui::RichText::new(toggle_text).size(14.0).color(toggle_color))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text(toggle_tip).clicked() {
                                    toggle_id = Some(job.id);
                                }

                                if ui.add(egui::Button::new(egui::RichText::new("▶").size(12.0).color(Theme::TEXT_MUTED))
                                    .fill(egui::Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                    .corner_radius(6.0).min_size(egui::vec2(28.0, 28.0))
                                ).on_hover_text("Run now").clicked() {
                                    run_id = Some(job.id);
                                }

                                ui.add_space(16.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(110.0, 36.0),
                                    egui::Layout::top_down(egui::Align::Max),
                                    |ui| {
                                        ui.label(egui::RichText::new("LAST RUN").size(9.0).color(Theme::TEXT_DIM));
                                        let last = job.last_run
                                            .map(|dt| time_ago(&dt))
                                            .unwrap_or_else(|| "—".into());
                                        let last_color = match &job.last_status {
                                            Some(RunStatus::Success) => Theme::SUCCESS,
                                            Some(RunStatus::Error)   => Theme::ERROR,
                                            _                        => Theme::TEXT_MUTED,
                                        };
                                        ui.label(egui::RichText::new(last).size(12.0).color(last_color));
                                    },
                                );
                            });
                        });

                        if is_selected {
                            ui.add_space(8.0);
                            ui.separator();
                            ui.add_space(4.0);

                            // Command block
                            egui::Frame::new()
                                .fill(Theme::BG)
                                .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                                .corner_radius(6.0)
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(egui::RichText::new("$").size(12.0).color(Theme::TEXT_DIM));
                                        ui.label(egui::RichText::new(&job.command).size(12.0).color(Theme::TEXT_MUTED));
                                    });
                                    // Show plist path for launchd jobs
                                    if let JobSource::Launchd { plist_path, .. } = &job.source {
                                        ui.label(egui::RichText::new(plist_path).size(10.0).color(Theme::TEXT_DIM));
                                    }
                                });

                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("RUN HISTORY").size(10.0).color(Theme::TEXT_DIM));
                            ui.add_space(4.0);

                            if job.logs.is_empty() {
                                ui.label(egui::RichText::new("No runs yet").size(12.0).color(Theme::TEXT_DIM));
                            }

                            for (i, log) in job.logs.iter().enumerate() {
                                let log_bg = if i == 0 {
                                    egui::Color32::from_rgba_premultiplied(26, 27, 35, 128)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                egui::Frame::new()
                                    .fill(log_bg)
                                    .corner_radius(4.0)
                                    .inner_margin(egui::Margin::symmetric(10, 6))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let log_color = match log.status {
                                                RunStatus::Success => Theme::SUCCESS,
                                                RunStatus::Error   => Theme::ERROR,
                                                RunStatus::Running => Theme::ACCENT,
                                            };
                                            let (dot_r, _) = ui.allocate_exact_size(
                                                egui::vec2(8.0, 8.0), egui::Sense::hover());
                                            ui.painter().circle_filled(dot_r.center(), 3.5, log_color);

                                            ui.vertical(|ui| {
                                                ui.horizontal(|ui| {
                                                    let ts = log.timestamp.with_timezone(&Local)
                                                        .format("%d %b %H:%M").to_string();
                                                    ui.label(egui::RichText::new(ts).size(11.0).color(Theme::TEXT_DIM));
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(egui::RichText::new(&log.duration).size(11.0).color(Theme::TEXT_DIM));
                                                    });
                                                });
                                                let out_color = if log.status == RunStatus::Error {
                                                    egui::Color32::from_rgb(252, 165, 165)
                                                } else {
                                                    Theme::TEXT_MUTED
                                                };
                                                ui.label(egui::RichText::new(&log.output).size(12.0).color(out_color));
                                            });
                                        });
                                    });
                            }
                        }
                    });

                if response.response.clicked() {
                    click_id = Some(job.id);
                }
                ui.add_space(4.0);
            }
        });

        // Apply mutations after the borrow ends
        if let Some(id) = click_id {
            self.selected = if self.selected == Some(id) { None } else { Some(id) };
        }

        if let Some(id) = toggle_id {
            let source = self.jobs.iter().find(|j| j.id == id).map(|j| j.source.clone());
            match source {
                Some(JobSource::Cron) => {
                    if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                        job.enabled = !job.enabled;
                    }
                    self.save_crontab();
                }
                Some(JobSource::Launchd { .. }) => self.toggle_launchd(id),
                None => {}
            }
        }

        if let Some(id) = run_id {
            if let Some(job) = self.jobs.iter_mut().find(|j| j.id == id) {
                let start = std::time::Instant::now();
                let output = Command::new("sh").arg("-c").arg(&job.command).output();
                let elapsed = start.elapsed();
                let duration_str = if elapsed.as_secs() > 0 {
                    format!("{}s", elapsed.as_secs())
                } else {
                    format!("{}ms", elapsed.as_millis())
                };
                let (status, out_text) = match output {
                    Ok(out) => {
                        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                        let text = if stdout.trim().is_empty() { stderr } else { stdout };
                        let text = text.trim().to_string();
                        if out.status.success() {
                            (RunStatus::Success, if text.is_empty() { "Completed successfully.".into() } else { text })
                        } else {
                            (RunStatus::Error, if text.is_empty() { "Command failed.".into() } else { text })
                        }
                    }
                    Err(e) => (RunStatus::Error, format!("Failed to execute: {}", e)),
                };
                let log = RunLog {
                    timestamp: Utc::now(),
                    status: status.clone(),
                    duration: duration_str.clone(),
                    output: out_text,
                };
                job.last_run = Some(Utc::now());
                job.last_status = Some(status);
                job.last_duration = Some(duration_str);
                job.logs.insert(0, log);
            }
        }

        if let Some(id) = delete_id {
            let source = self.jobs.iter().find(|j| j.id == id).map(|j| j.source.clone());
            match source {
                Some(JobSource::Cron) => {
                    self.jobs.retain(|j| j.id != id);
                    if self.selected == Some(id) { self.selected = None; }
                    self.save_crontab();
                }
                Some(JobSource::Launchd { plist_path, .. }) => {
                    match delete_launchd_job(&plist_path) {
                        Ok(_) => {
                            self.jobs.retain(|j| j.id != id);
                            if self.selected == Some(id) { self.selected = None; }
                            self.status_msg = Some("Launchd job deleted.".into());
                        }
                        Err(e) => self.status_msg = Some(format!("Error: {e}")),
                    }
                }
                None => {}
            }
        }
    }

    fn render_add_dialog(&mut self, ctx: &egui::Context) {
        let mut open = true;
        egui::Window::new("New Job")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(420.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);

                // Source selector
                ui.label(egui::RichText::new("SOURCE").size(10.0).color(Theme::TEXT_DIM));
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.new_source_is_launchd, false, "Cron");
                    ui.radio_value(&mut self.new_source_is_launchd, true,  "Launchd");
                });
                ui.add_space(6.0);

                ui.label(egui::RichText::new("NAME").size(10.0).color(Theme::TEXT_DIM));
                ui.text_edit_singleline(&mut self.new_name);
                ui.add_space(6.0);

                if self.new_source_is_launchd {
                    ui.label(egui::RichText::new("LABEL").size(10.0).color(Theme::TEXT_DIM));
                    ui.text_edit_singleline(&mut self.new_label);
                    ui.label(egui::RichText::new("e.g. com.user.my-task").size(10.0).color(Theme::TEXT_DIM));
                    ui.add_space(6.0);
                }

                ui.label(egui::RichText::new("SCHEDULE").size(10.0).color(Theme::TEXT_DIM));
                ui.text_edit_singleline(&mut self.new_schedule);
                ui.label(egui::RichText::new("e.g. 0 7 * * *").size(10.0).color(Theme::TEXT_DIM));
                ui.add_space(6.0);

                ui.label(egui::RichText::new("COMMAND").size(10.0).color(Theme::TEXT_DIM));
                ui.text_edit_singleline(&mut self.new_command);
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Cancel")
                        .fill(Theme::PANEL)
                        .stroke(egui::Stroke::new(1.0, Theme::BORDER))
                        .min_size(egui::vec2(80.0, 30.0))
                    ).clicked() {
                        self.show_add_dialog = false;
                        self.clear_form();
                    }

                    let label_ok = !self.new_source_is_launchd || !self.new_label.is_empty();
                    let can_add  = !self.new_name.is_empty()
                        && !self.new_schedule.is_empty()
                        && !self.new_command.is_empty()
                        && label_ok;

                    if ui.add(egui::Button::new(egui::RichText::new("Add Job").color(egui::Color32::WHITE))
                        .fill(if can_add { Theme::ACCENT } else { Theme::BORDER })
                        .min_size(egui::vec2(80.0, 30.0))
                    ).clicked() && can_add {
                        let next_id = self.jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;

                        if self.new_source_is_launchd {
                            match write_launchd_plist(&self.new_label, &self.new_command, &self.new_schedule) {
                                Ok(plist_path) => {
                                    self.jobs.push(CronJob {
                                        id: next_id,
                                        name: self.new_name.clone(),
                                        schedule: self.new_schedule.clone(),
                                        command: self.new_command.clone(),
                                        enabled: true,
                                        source: JobSource::Launchd {
                                            label: self.new_label.clone(),
                                            plist_path,
                                        },
                                        last_run: None,
                                        last_status: None,
                                        last_duration: None,
                                        logs: Vec::new(),
                                    });
                                    self.show_add_dialog = false;
                                    self.clear_form();
                                    self.status_msg = Some("Launchd job created.".into());
                                }
                                Err(e) => self.status_msg = Some(format!("Error: {e}")),
                            }
                        } else {
                            self.jobs.push(CronJob {
                                id: next_id,
                                name: self.new_name.clone(),
                                schedule: self.new_schedule.clone(),
                                command: self.new_command.clone(),
                                enabled: true,
                                source: JobSource::Cron,
                                last_run: None,
                                last_status: None,
                                last_duration: None,
                                logs: Vec::new(),
                            });
                            self.show_add_dialog = false;
                            self.clear_form();
                            self.save_crontab();
                        }
                    }
                });
            });

        if !open {
            self.show_add_dialog = false;
            self.clear_form();
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    let use_mock = std::env::args().any(|a| a == "--mock");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 620.0])
            .with_title("Scheduler UI"),
        ..Default::default()
    };

    eframe::run_native(
        "Scheduler UI",
        options,
        Box::new(move |_cc| {
            let app = if use_mock {
                SchedulerApp::new(sample_jobs(), Vec::new(), true)
            } else {
                let (mut jobs, preamble) = parse_crontab();
                let launchd = parse_launchd_agents(jobs.len());
                jobs.extend(launchd);
                let jobs = if jobs.is_empty() { sample_jobs() } else { jobs };
                SchedulerApp::new(jobs, preamble, false)
            };
            Ok(Box::new(app))
        }),
    )
}
