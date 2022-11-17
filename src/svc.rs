use crate::api;
use chrono::{DateTime, Duration, TimeZone, Utc};

const CREATED_WITH: &str = "github.com/blachniet/tgl";

pub struct Client {
    c: api::Client,
    get_now: fn() -> DateTime<Utc>,
    project_cache: elsa::map::FrozenMap<(i64, i64), Box<Project>>,
}

impl Client {
    pub fn new(token: String, get_now: fn() -> DateTime<Utc>) -> Result<Self> {
        Ok(Self {
            c: api::Client::new(token)?,
            get_now,
            project_cache: elsa::map::FrozenMap::new(),
        })
    }

    pub fn get_latest_entries(&self) -> Result<Vec<TimeEntry>> {
        let api_entries = self.c.get_time_entries(None)?;
        let entries: Result<Vec<_>> = api_entries
            .into_iter()
            .map(|e| self.build_time_entry(e))
            .collect();

        entries
    }

    fn build_time_entry(&self, api_entry: api::TimeEntry) -> Result<TimeEntry> {
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
            project_id,
            project_name: project.map(|p| p.name.to_string()),
            start,
            stop,
            workspace_id: api_entry.workspace_id.as_i64().unwrap(),
        })
    }

    pub fn start_time_entry(
        &self,
        workspace_id: i64,
        project_id: Option<i64>,
        description: Option<&str>,
    ) -> Result<TimeEntry> {
        let now = (self.get_now)();
        let api_entry = self.c.create_time_entry(api::NewTimeEntry {
            created_with: CREATED_WITH.to_string(),
            description: description.map(|d| d.to_string()),
            duration: (-now.timestamp()).into(),
            project_id: project_id.map(|i| i.into()),
            start: now.to_rfc3339(),
            stop: None,
            task_id: None,
            workspace_id: workspace_id.into(),
        })?;
        let entry = self.build_time_entry(api_entry)?;

        Ok(entry)
    }

    pub fn stop_current_time_entry(&self) -> Result<Option<TimeEntry>> {
        if let Some(api_entry) = self.c.get_current_entry()? {
            let api_entry = self
                .c
                .stop_time_entry(&api_entry.workspace_id, &api_entry.id)?;
            let entry = self.build_time_entry(api_entry)?;

            Ok(Some(entry))
        } else {
            Ok(None)
        }
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

    fn get_project(&self, workspace_id: i64, project_id: i64) -> Result<Option<&Project>> {
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

    pub fn get_projects(&self, workspace_id: i64) -> Result<Vec<Project>> {
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

    pub fn get_workspaces(&self) -> Result<Vec<Workspace>> {
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
pub enum Error {
    Reqwest(reqwest::Error),
    ChronoParse(chrono::ParseError),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Reqwest(e) => write!(f, "reqwest: {}", e),
            Error::ChronoParse(e) => write!(f, "chrono: {}", e),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}

impl From<chrono::ParseError> for Error {
    fn from(e: chrono::ParseError) -> Self {
        Error::ChronoParse(e)
    }
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct TimeEntry {
    pub description: Option<String>,
    pub duration: Duration,
    pub is_running: bool,
    pub project_id: Option<i64>,
    pub project_name: Option<String>,
    pub start: Option<DateTime<Utc>>,
    pub stop: Option<DateTime<Utc>>,
    pub workspace_id: i64,
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
