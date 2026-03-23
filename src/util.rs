use chrono::{DateTime, Duration, Utc};

use crate::models::{CronJob, JobSource, RunLog, RunStatus};

pub fn extract_name(command: &str) -> String {
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

pub fn title_case(s: &str) -> String {
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

pub fn human_schedule(expr: &str) -> String {
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

pub fn time_ago(dt: &DateTime<Utc>) -> String {
    let diff = Utc::now().signed_duration_since(*dt);
    let mins = diff.num_minutes();
    if mins < 1  { return "just now".into(); }
    if mins < 60 { return format!("{}m ago", mins); }
    let hrs = diff.num_hours();
    if hrs < 24  { return format!("{}h ago", hrs); }
    format!("{}d ago", diff.num_days())
}

pub fn sample_jobs() -> Vec<CronJob> {
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
