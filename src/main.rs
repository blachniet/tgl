use chrono::{DateTime, Duration, Local, Utc};
use clap::{Parser, Subcommand};
use std::{env, process::exit};
use tgl_cli::{
    error::Error,
    svc::{Client, TimeEntry},
};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start a new time entry
    Start,
    /// Stop the current time entry
    Stop,
}

fn main() {
    let cli = Cli::parse();
    let result = match &cli.command {
        Some(Command::Start) => run_start(),
        Some(Command::Stop) => {
            todo!("implement stop");
        }
        None => run_status(),
    };

    if let Err(err) = result {
        if !err.message.is_empty() {
            println!("{}", err.message);
        }

        exit(1);
    }
}

fn get_client() -> Result<Client, Error> {
    let token = get_api_token()?;

    Client::new(token, Utc::now).map_err(|e| Error {
        message: format!("Could not connect to Toggl: {}", e),
    })
}

fn get_api_token() -> Result<String, Error> {
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
                    .interact()
                    .map_err(|e| Error::new(format!("Couldn't read the password: {}", e).into()))?;

                entry.set_password(&token).map_err(|e| {
                    Error::new(
                        format!("Couldn't save the API token your keyring/keychain: {}", e).into(),
                    )
                })?;

                Ok(token)
            }
            _ => Err(Error::new(format!(
                "Couldn't read from your keyring/keychain: {}",
                err
            ))),
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

fn run_status() -> Result<(), Error> {
    let client = get_client()?;
    let today = Local::today().and_hms(0, 0, 0);
    let tomorrow = Local::today().succ().and_hms(0, 0, 0);
    let mut latest_entries = client.get_latest_entries().map_err(map_svc_err)?;
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

fn run_start() -> Result<(), Error> {
    let client = get_client()?;
    let workspaces = client.get_workspaces().map_err(map_svc_err)?;
    let workspace_names: Vec<_> = workspaces.iter().map(|w| w.name.to_string()).collect();
    let workspace_idx = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select a workspace:")
        .items(&workspace_names)
        .interact_on_opt(&dialoguer::console::Term::stderr())
        .map_err(map_input_err)?
        .ok_or_else(|| Error::new("You must select a workspace.".to_string()))?;

    let workspace = &workspaces[workspace_idx];
    let projects = client.get_projects(workspace.id).map_err(map_svc_err)?;
    let projects: Vec<_> = projects.iter().filter(|p| p.active).collect();
    let project_names: Vec<_> = projects.iter().map(|p| p.name.to_string()).collect();
    let project_idx = dialoguer::Select::with_theme(&dialoguer::theme::ColorfulTheme::default())
        .with_prompt("Select a project or press 'q' to skip:")
        .items(&project_names)
        .interact_on_opt(&dialoguer::console::Term::stderr())
        .map_err(map_input_err)?;

    let project_id = project_idx.map(|i| projects[i].id);
    let description: String = dialoguer::Input::new()
        .with_prompt("Enter a description or press 'q' to skip:")
        .default("".into())
        .interact_text()
        .map_err(map_input_err)?;

    client
        .start_time_entry(workspace.id, project_id, Some(&description))
        .map_err(map_svc_err)?;

    Ok(())
}

fn map_svc_err(e: Box<dyn std::error::Error>) -> Error {
    Error::new(format!("Couldn't connect to Toggl: {e}"))
}

fn map_input_err(e: std::io::Error) -> Error {
    Error::new(format!("Couldn't read that input: {e}"))
}
