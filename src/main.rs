use chrono::{DateTime, Duration, Local, Utc};
use std::{env, error};
use tgl_cli::svc::TimeEntry;

fn main() -> Result<(), Box<dyn error::Error>> {
    let token = get_api_token()?;
    let client = tgl_cli::svc::Client::new(token, Utc::now)?;

    let today = Local::today().and_hms(0, 0, 0);
    let tomorrow = Local::today().succ().and_hms(0, 0, 0);
    let mut latest_entries = client.get_latest_entries()?;
    latest_entries.sort_unstable_by_key(|e| e.start);

    let mut is_running = false;
    let mut dur_today = Duration::zero();
    for entry in latest_entries.iter().filter(|e| {
        if let Some(start) = e.start {
            if start >= today && start < tomorrow {
                return true;
            }
        }

        if let Some(stop) = e.stop {
            if stop >= today && stop < tomorrow {
                return true;
            }
        }

        false
    }) {
        println!("{}", fmt_entry(entry));
        dur_today = dur_today + entry.duration;
        is_running = is_running || entry.is_running;
    }

    if !is_running {
        println!("🤷 No timers running");
    }

    let dur_today = fmt_duration(dur_today);
    println!("\n⏱  {dur_today} logged today");

    Ok(())
}

fn get_api_token() -> Result<String, Box<dyn error::Error>> {
    // Look for the token in an environment variable.
    let token = env::var("TOGGL_API_TOKEN");
    if let Ok(token) = token {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Look for the token in the keyring.
    let entry = keyring::Entry::new("github.com/blachniet/tgl", "api_token");
    let token = match entry.get_password() {
        Ok(token) => Ok(token),
        Err(err) => match err {
            keyring::Error::NoEntry => {
                let token = dialoguer::Password::new()
                    .with_prompt("Enter your API token from https://track.toggl.com/profile")
                    .with_confirmation("Confirm token", "Tokens don't match")
                    .interact()?;

                entry.set_password(&token)?;
                Ok(token)
            }
            _ => Err(err),
        },
    }?;

    Ok(token)
}

fn fmt_entry(entry: &TimeEntry) -> String {
    let icon = match entry.is_running {
        true => "🏃",
        false => "- ",
    };
    let duration = fmt_duration(entry.duration);
    let project_name = entry
        .project_name
        .as_ref()
        .map_or("<no project>".to_string(), |n| n.to_string());
    let description = entry.description.as_ref().map_or("".to_string(), |d| {
        if d.is_empty() {
            "".to_string()
        } else {
            format!("- {d}")
        }
    });
    let start_stop = fmt_start_stop(entry);

    format!("{icon} {duration} {project_name} {description} {start_stop}")
}

fn fmt_duration(dur: Duration) -> String {
    let (hours, minutes, seconds) = get_duration_parts(dur);
    format!("{hours}:{minutes:02}:{seconds:02}")
}

fn fmt_start_stop(entry: &TimeEntry) -> String {
    if let Some(start) = entry.start {
        let start: DateTime<Local> = DateTime::from(start);
        if let Some(stop) = entry.stop {
            let stop: DateTime<Local> = DateTime::from(stop);
            format!("{start} / {stop}")
        } else {
            format!("{start} / ...")
        }
    } else {
        "".to_string()
    }
}

fn get_duration_parts(dur: Duration) -> (i64, i64, i64) {
    let minutes = (dur - Duration::hours(dur.num_hours())).num_minutes();
    let seconds = (dur - Duration::minutes(dur.num_minutes())).num_seconds();

    (dur.num_hours(), minutes, seconds)
}
