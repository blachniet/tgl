use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Datelike, Days, Duration, Local, TimeZone, Utc};
use clap::{Parser, Subcommand};
use dialoguer::theme::Theme;
use std::env;
use tgl_cli::svc::{Client, TimeEntry};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Get the current status of Toggl timers for today
    Status,
    /// Start a new time entry
    Start,
    /// Stop the current time entry
    Stop,
    /// Restart the latest time entry
    Restart,
    /// Delete the Toggl API token saved in the keyring/keychain
    DeleteApiToken,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Command::Status) => run_status(),
        Some(Command::Start) => run_start(),
        Some(Command::Stop) => run_stop(),
        Some(Command::Restart) => run_restart(),
        Some(Command::DeleteApiToken) => run_delete_api_token(),
        None => run_status(),
    }
}

fn get_client() -> Result<Client> {
    let token = get_api_token()?;

    Client::new(token, Utc::now).context("Failed to create Toggle API client")
}

fn keyring_entry() -> keyring::Entry {
    keyring::Entry::new("github.com/blachniet/tgl", "api_token")
}

fn get_api_token() -> Result<String> {
    // Look for the token in an environment variable.
    let token = env::var("TOGGL_API_TOKEN");
    if let Ok(token) = token {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    // Look for the token in the keyring.
    let entry = keyring_entry();
    let result = entry.get_password();
    let token = match result {
        Ok(token) => Ok(token),
        Err(ref err) => match err {
            keyring::Error::NoEntry => {
                let token = dialoguer::Password::new()
                    .with_prompt("Enter your API token from https://track.toggl.com/profile")
                    .with_confirmation("Confirm token", "Tokens don't match")
                    .interact()
                    .context("Failed to read API token from keyring/keychain")?;

                entry
                    .set_password(&token)
                    .context("Failed to save the API token to the keyring/keychain")?;

                Ok(token)
            }
            _ => result.context("Failed to read from your keyring/keychain"),
        },
    }?;

    Ok(token)
}

fn println_entry(entry: &TimeEntry) {
    println!(
        "{} ({}) [{}] {}",
        fmt_duration(entry.duration),
        fmt_start_stop(entry),
        entry.project_name.as_ref().unwrap_or(&"".to_string()),
        entry.description.as_ref().unwrap_or(&"".to_string()),
    );
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
            format!(
                "{} - {}",
                start.time().format("%H:%M"),
                stop.time().format("%H:%M")
            )
        } else {
            format!("{} - ⏳:⏳", start.time().format("%H:%M"))
        }
    } else {
        String::new()
    }
}

fn get_duration_parts(dur: Duration) -> (i64, i64, i64) {
    let minutes = (dur - Duration::hours(dur.num_hours())).num_minutes();
    let seconds = (dur - Duration::minutes(dur.num_minutes())).num_seconds();

    (dur.num_hours(), minutes, seconds)
}

fn run_status() -> Result<()> {
    let client = get_client()?;
    let now = Local::now();
    let today = Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .unwrap();
    let tomorrow = today.checked_add_days(Days::new(1)).unwrap();
    let mut latest_entries = client
        .get_latest_entries()
        .context("Failed to retrieve time entries")?;
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
        println_entry(entry);
        dur_today += entry.duration;
        is_running = is_running || entry.is_running;
    }

    println!();
    print!("⏱  {} logged today.", fmt_duration(dur_today));

    if is_running {
        let target_dur = Duration::hours(8);
        let dur_remaining = target_dur - dur_today;
        let target_time = (Local::now() + dur_remaining).time();
        println!(
            " You'll reach {} logged at {}.",
            fmt_duration(target_dur),
            target_time.format("%H:%M")
        );
    } else {
        println!();
    }

    Ok(())
}

fn run_start() -> Result<()> {
    let client = get_client()?;
    let workspaces = client
        .get_workspaces()
        .context("Failed to retrieve workspaces")?;
    let workspace_names: Vec<_> = workspaces.iter().map(|w| w.name.to_string()).collect();
    let workspace_idx = match workspace_names.len() {
        0 => Err(anyhow!("No Toggl workspaces found")),
        1 => {
            let mut buf = String::new();
            dialoguer::theme::ColorfulTheme::default().format_input_prompt_selection(
                &mut buf,
                "Using only workspace",
                &workspace_names[0],
            )?;
            dialoguer::console::Term::stderr().write_line(&buf)?;

            Ok(0)
        }
        _ => dialoguer::FuzzySelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select a workspace")
            .items(&workspace_names)
            .default(0)
            .interact_on_opt(&dialoguer::console::Term::stderr())
            .context("Failed to read workspace input")?
            .ok_or_else(|| anyhow!("You must select a workspace")),
    }?;

    let workspace = &workspaces[workspace_idx];
    let projects = client
        .get_projects(workspace.id)
        .context("Failed to get projects")?;
    let projects: Vec<_> = projects.iter().filter(|p| p.active).collect();
    let project_names: Vec<_> = projects.iter().map(|p| p.name.to_string()).collect();
    let project_idx =
        dialoguer::FuzzySelect::with_theme(&dialoguer::theme::ColorfulTheme::default())
            .with_prompt("Select a project or press 'Esc' to skip")
            .items(&project_names)
            .interact_on_opt(&dialoguer::console::Term::stderr())
            .context("Failed to read project selection")?;

    let project_id = project_idx.map(|i| projects[i].id);
    let description: String = dialoguer::Input::new()
        .with_prompt("Enter a description (optional)")
        .allow_empty(true)
        .interact_text()
        .context("Failed to read description input")?;

    client
        .start_time_entry(workspace.id, project_id, Some(&description))
        .context("Failed to start time entry")?;

    run_status()
}

fn run_stop() -> Result<()> {
    let client = get_client()?;
    if client
        .stop_current_time_entry()
        .context("Failed to stop current time entry")?
        .is_none()
    {
        println!("🤷 No timers running\n");
    }

    run_status()
}

fn run_restart() -> Result<()> {
    let client = get_client()?;
    let recent_entries = client
        .get_latest_entries()
        .context("Failed to retrieve latest time entries")?;
    if let Some(last_entry) = recent_entries.first() {
        client
            .start_time_entry(
                last_entry.workspace_id,
                last_entry.project_id,
                last_entry.description.as_deref(),
            )
            .context("Failed to start time entry")?;
    } else {
        bail!("🤷 No recent entries to restart");
    }

    run_status()
}

fn run_delete_api_token() -> Result<()> {
    keyring_entry()
        .delete_password()
        .context("Failed to delete API token from keyring/keychain")
}
