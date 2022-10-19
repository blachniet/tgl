use chrono::{Duration, TimeZone, Utc};
use std::collections::HashSet;
use std::env;
use std::error;

fn main() -> Result<(), Box<dyn error::Error>> {
    let now = Utc::now();
    let token = env::var("TOGGL_API_TOKEN")?;
    let client = togglapi::Client::new(token)?;

    if let Some(current_entry) = client.get_current_entry()? {
        if let Some(negative_start_epoch) = current_entry.duration.as_ref().map(|d| d.as_i64()).flatten() {
            let start = Utc.timestamp(-1*negative_start_epoch, 0);
            let duration = now - start;
            let (hours, minutes, seconds) = get_duration_parts(duration);
            println!("🏃 {}h{}m{}s", hours, minutes, seconds);
        } else {
            eprintln!("Error: cannot parse {:?} as i64", current_entry.duration);
            std::process::exit(1);
        }
    } else {
        println!("🧍 No timers running");
    }

    let recent_entries = client.get_recent_entries()?;
    println!("\nrecent entries = {:?}", recent_entries);

    let recent_workspace_ids: HashSet<_> = recent_entries
        .into_iter()
        .filter_map(|e| e.workspace_id)
        .collect();
    println!("\nrecent workspace ids = {:?}", recent_workspace_ids);

    let recent_projects: Result<Vec<_>, _> = recent_workspace_ids
        .iter()
        .map(|wid| client.get_projects(wid))
        .collect();
    println!("\nrecent projects = {:?}", recent_projects?);

    Ok(())
}

fn get_duration_parts(dur: Duration) -> (i64, i64, i64) {
    let minutes = (dur - Duration::hours(dur.num_hours())).num_minutes();
    let seconds = (dur - Duration::minutes(dur.num_minutes())).num_seconds();
    
    (dur.num_hours(), minutes, seconds)
}

mod togglapi {
    use reqwest::header;
    use serde::Deserialize;
    use serde_json::Number;

    pub struct Client {
        c: reqwest::blocking::Client,
        token: String,
    }

    impl Client {
        pub fn new(token: String) -> Result<Self, reqwest::Error> {
            let mut headers = header::HeaderMap::new();

            // Toggl API docs indicate that we should always include the JSON
            // content type header.
            headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json"),
            );

            Ok(Client {
                c: reqwest::blocking::Client::builder()
                    .default_headers(headers)
                    .build()?,
                token,
            })
        }

        pub fn get_current_entry(&self) -> Result<Option<TimeEntry>, Box<dyn std::error::Error>> {
            let current_entry: Option<TimeEntry> = self
                .c
                .get("https://api.track.toggl.com/api/v9/me/time_entries/current")
                .basic_auth(&self.token, Some("api_token"))
                .send()?
                .json()?;

            Ok(current_entry)
        }

        pub fn get_recent_entries(&self) -> Result<Vec<TimeEntry>, Box<dyn std::error::Error>> {
            let recent_entries: Vec<TimeEntry> = self
                .c
                .get("https://api.track.toggl.com/api/v9/me/time_entries")
                .basic_auth(&self.token, Some("api_token"))
                .send()?
                .json()?;

            Ok(recent_entries)
        }

        pub fn get_projects(
            &self,
            workspace_id: &Number,
        ) -> Result<Vec<Project>, Box<dyn std::error::Error>> {
            Ok(self
                .c
                .get(format!(
                    "https://api.track.toggl.com/api/v9/workspaces/{workspace_id}/projects"
                ))
                .basic_auth(&self.token, Some("api_token"))
                .send()?
                .json()?)
        }
    }

    #[derive(Deserialize, Debug)]
    pub struct TimeEntry {
        pub duration: Option<Number>,
        pub id: Option<Number>,
        pub project_id: Option<Number>,
        pub start: Option<String>,
        pub stop: Option<String>,
        pub task_id: Option<Number>,
        pub workspace_id: Option<Number>,
    }

    #[derive(Deserialize, Debug)]
    pub struct Project {
        pub client_id: Option<Number>,
        pub id: Option<Number>,
        pub name: Option<String>,
        pub workspace_id: Option<Number>,
    }
}
