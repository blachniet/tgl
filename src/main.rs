use chrono::{DateTime, Duration, Local, Utc};
use std::{env, error};
use togglsvc::TimeEntry;

fn main() -> Result<(), Box<dyn error::Error>> {
    let token = get_api_token()?;
    let client = togglsvc::Client::new(token, Utc::now)?;

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

        pub fn get_latest_entries(&self) -> Result<Vec<TimeEntry>, Box<dyn std::error::Error>> {
            let api_entries = self.c.get_time_entries(None)?;
            let entries: Result<Vec<_>, _> = api_entries
                .into_iter()
                .map(|e| self.build_time_entry(e))
                .collect();

            entries
        }

        fn build_time_entry(
            &self,
            api_entry: togglapi::TimeEntry,
        ) -> Result<TimeEntry, Box<dyn std::error::Error>> {
            let project_id = api_entry.project_id.map(|pid| pid.as_i64().unwrap());
            let project = match project_id {
                Some(pid) => self.get_project(api_entry.workspace_id.as_i64().unwrap(), pid)?,
                None => None,
            };
            let (duration, is_running) = self.parse_duration(api_entry.duration);
            let start: Option<DateTime<Utc>> = match api_entry.start {
                Some(s) => Some(s.parse()?),
                None => None,
            };
            let stop: Option<DateTime<Utc>> = match api_entry.stop {
                Some(s) => Some(s.parse()?),
                None => None,
            };

            Ok(TimeEntry {
                description: api_entry.description,
                duration,
                is_running,
                project_name: project.map(|p| p.name.to_string()),
                start,
                stop,
            })
        }

        /// Creates a [`chrono::Duration`] from a Toggle API duration.
        ///
        /// Returns a tuple containing the duration value and bool. If the bool
        /// is `true`, then the associated timer was running. If the bool is
        /// `false`, then the associated timer was not running.
        ///
        /// Panics if `duration` cannot be represented as an `i64`.
        fn parse_duration(&self, duration: serde_json::Number) -> (Duration, bool) {
            let duration = duration.as_i64().unwrap();
            if duration < 0 {
                (
                    // Running entry is represented as the negative epoch timestamp
                    // of the start time.
                    (self.get_now)() - Utc.timestamp(-duration, 0),
                    true,
                )
            } else {
                (Duration::seconds(duration), false)
            }
        }

        fn get_project(
            &self,
            workspace_id: i64,
            project_id: i64,
        ) -> Result<Option<&Project>, Box<dyn std::error::Error>> {
            let key = (workspace_id, project_id);
            if let Some(project) = self.project_cache.get(&key) {
                return Ok(Some(project));
            }

            let workspace_id_num = workspace_id.into();
            let projects = self.c.get_projects(&workspace_id_num)?;
            for p in projects {
                self.project_cache.insert(
                    (workspace_id, p.id.as_i64().expect("parse number as i64")),
                    Box::new(Project {
                        active: p.active,
                        id: p.id.as_i64().unwrap(),
                        name: p.name,
                    }),
                );
            }

            Ok(self.project_cache.get(&key))
        }

        pub fn get_projects(
            &self,
            workspace_id: i64,
        ) -> Result<Vec<Project>, Box<dyn std::error::Error>> {
            let api_projects = self.c.get_projects(&workspace_id.into())?;
            let mut projects = Vec::new();

            for p in api_projects {
                self.project_cache.insert(
                    (workspace_id, p.id.as_i64().expect("parse number as i64")),
                    Box::new(Project {
                        active: p.active,
                        id: p.id.as_i64().unwrap(),
                        name: p.name.to_string(),
                    }),
                );

                projects.push(Project {
                    active: p.active,
                    id: p.id.as_i64().unwrap(),
                    name: p.name,
                });
            }

            Ok(projects)
        }

        pub fn get_workspaces(&self) -> Result<Vec<Workspace>, Box<dyn std::error::Error>> {
            let workspaces = self.c.get_workspaces()?;
            Ok(workspaces
                .into_iter()
                .map(|w| Workspace {
                    id: w.id.as_i64().unwrap(),
                    name: w.name,
                })
                .collect())
        }
    }

    #[derive(Debug)]
    pub struct TimeEntry {
        pub description: Option<String>,
        pub duration: Duration,
        pub is_running: bool,
        pub project_name: Option<String>,
        pub start: Option<DateTime<Utc>>,
        pub stop: Option<DateTime<Utc>>,
    }

    #[derive(Debug)]
    pub struct Project {
        pub active: bool,
        pub id: i64,
        pub name: String,
    }

    #[derive(Debug)]
    pub struct Workspace {
        pub id: i64,
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

        pub fn get_time_entries(
            &self,
            start_end_dates: Option<(NaiveDate, NaiveDate)>,
        ) -> Result<Vec<TimeEntry>, Box<dyn std::error::Error>> {
            let base_url = "https://api.track.toggl.com/api/v9/me/time_entries";
            let url = match start_end_dates {
                Some((start_date, end_date)) => {
                    format!("{base_url}?start_date={start_date}&end_date={end_date}")
                }
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

        pub fn get_workspaces(&self) -> Result<Vec<Workspace>, Box<dyn std::error::Error>> {
            Ok(self
                .c
                .get(format!("https://api.track.toggl.com/api/v9/workspaces"))
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
        pub active: bool,
        pub client_id: Option<Number>,
        pub id: Number,
        pub name: String,
        pub workspace_id: Number,
    }

    #[derive(Deserialize, Debug)]
    pub struct Workspace {
        pub id: Number,
        pub name: String,
    }
}
