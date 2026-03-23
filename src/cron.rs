use std::io::Write;
use std::process::{Command, Stdio};

use crate::models::{CronJob, JobSource};
use crate::util::extract_name;

pub fn parse_crontab() -> (Vec<CronJob>, Vec<String>) {
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

pub fn try_parse_job(id: usize, line: &str, enabled: bool, source: JobSource) -> Option<CronJob> {
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

pub fn write_crontab(preamble: &[String], jobs: &[CronJob]) -> Result<(), String> {
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
