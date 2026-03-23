use std::process::Command;

use crate::models::{CronJob, JobSource};
use crate::util::title_case;

pub fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(std::path::PathBuf::from)
}

pub fn is_launchd_loaded(label: &str) -> bool {
    Command::new("launchctl")
        .args(["list", label])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn launchctl_load(plist_path: &str) -> Result<(), String> {
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

pub fn launchctl_unload(plist_path: &str) -> Result<(), String> {
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

pub fn delete_launchd_job(plist_path: &str) -> Result<(), String> {
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
            "* * * * *".into()
        }
    }
}

pub fn parse_launchd_agents(id_offset: usize) -> Vec<CronJob> {
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

pub fn write_launchd_plist(label: &str, command: &str, schedule: &str) -> Result<String, String> {
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
