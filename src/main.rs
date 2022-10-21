use chrono::{Duration, Utc};
use std::env;
use std::error;

fn main() -> Result<(), Box<dyn error::Error>> {
    let entry = keyring::Entry::new("github.com/blachniet/tgl", "api_token");
    let token = match entry.get_password() {
        Ok(token) => Ok(token),
        Err(err) => match err {
            keyring::Error::NoEntry => {
                let token = dialoguer::Password::new().with_prompt("Enter your API token from https://track.toggl.com/profile")
                    .with_confirmation("Confirm token", "Tokens don't match")
                    .interact()?;

                entry.set_password(&token)?;
                Ok(token)
            },
            _ => Err(err),
        },
    }?;

    let client = togglsvc::Client::new(token.to_string(), || Utc::now())?;

    if let Some(current_entry) = client.get_current_entry()? {
        let (hours, minutes, seconds) = get_duration_parts(current_entry.duration);
        let project_txt = current_entry.project_name.unwrap_or("<no project>".to_string());
        let description_txt = current_entry.description.unwrap_or("<no description>".to_string());
        println!("🏃 {hours}h{minutes}m{seconds}s {project_txt} - {description_txt}");
    } else {
        println!("🤷 No timers running");
    }

    Ok(())
}

fn get_duration_parts(dur: Duration) -> (i64, i64, i64) {
    let minutes = (dur - Duration::hours(dur.num_hours())).num_minutes();
    let seconds = (dur - Duration::minutes(dur.num_minutes())).num_seconds();
    
    (dur.num_hours(), minutes, seconds)
}

mod togglsvc {
    use super::togglapi;
    use chrono::{DateTime, Duration, TimeZone, Utc};

    pub struct Client {
        c: togglapi::Client,
        get_now: fn() -> DateTime<Utc>,
        project_cache: elsa::map::FrozenMap<(i64, i64), Box<Project>>,
    }

    impl Client {
        pub fn new(token: String, get_now: fn() -> DateTime<Utc>) -> Result<Self, reqwest::Error> {
            Ok(Self {
                c: togglapi::Client::new(token)?,
                get_now,
                project_cache: elsa::map::FrozenMap::new(),
            })
        }

        pub fn get_current_entry(&self) -> Result<Option<TimeEntry>, Box<dyn std::error::Error>>{
            if let Some(curr_entry) = self.c.get_current_entry()? {
                let project_id = curr_entry.project_id.map(|pid| pid.as_i64().unwrap());
                let project = match project_id {
                    Some(pid) => self.get_project(
                        curr_entry.workspace_id.as_i64().unwrap(),
                        pid,
                    )?,
                    None => None,
                };

                Ok(Some(TimeEntry {
                    description: curr_entry.description,
                    duration: self.toggl_to_chrono_duration(curr_entry.duration),
                    project_name: project.map(|p| p.name.to_string()),
                }))
            } else {
                Ok(None)
            }
        }

        fn toggl_to_chrono_duration(&self, duration: serde_json::Number) -> Duration {
            let duration = duration.as_i64().unwrap();
            if duration < 0 {
                // Running entry is represented as the negative epoch timestamp
                // of the start time.
                (self.get_now)() - Utc.timestamp(-1*duration, 0)
            } else {
                Duration::seconds(duration)
            }
        }

        fn get_project(&self, workspace_id: i64, project_id: i64) -> Result<Option<&Project>, Box<dyn std::error::Error>> {
            let key = (workspace_id, project_id);
            if let Some(project) = self.project_cache.get(&key) {
                return Ok(Some(project));
            }

            let workspace_id_num = workspace_id.into();
            let projects = self.c.get_projects(&workspace_id_num)?;
            for p in projects {
                self.project_cache.insert(
                    (workspace_id, p.id.as_i64().expect("parse number as i64")),
                    Box::new(Project{ name: p.name }),
                );
            }

            Ok(self.project_cache.get(&key))
        }
    }

    pub struct TimeEntry {
        pub description: Option<String>,
        pub duration: Duration,
        pub project_name: Option<String>,
    }

    pub struct Project {
        pub name: String,
    }
}

mod togglapi {
    use chrono::NaiveDate;
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

        pub fn get_time_entries(&self, start_end_dates: Option<(NaiveDate, NaiveDate)>) -> Result<Vec<TimeEntry>, Box<dyn std::error::Error>> {
            let base_url = "https://api.track.toggl.com/api/v9/me/time_entries";
            let url = match start_end_dates {
                Some((start_date, end_date)) => format!("{base_url}?start_date={start_date}&end_date={end_date}"),
                None => base_url.to_string(),
            };

            let recent_entries: Vec<TimeEntry> = self
                .c
                .get(url)
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
        pub description: Option<String>,
        pub duration: Number,
        pub id: Number,
        pub project_id: Option<Number>,
        pub start: Option<String>,
        pub stop: Option<String>,
        pub task_id: Option<Number>,
        pub workspace_id: Number,
    }

    #[derive(Deserialize, Debug)]
    pub struct Project {
        pub client_id: Option<Number>,
        pub id: Number,
        pub name: String,
        pub workspace_id: Number,
    }
}
