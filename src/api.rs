use chrono::NaiveDate;
use reqwest::header;
use serde::{Deserialize, Serialize};
use serde_json::Number;

static BASE_API_URL: &str = "https://api.track.toggl.com/api/v9";

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
    ) -> Result<Vec<TimeEntry>, reqwest::Error> {
        let url = match start_end_dates {
            Some((start_date, end_date)) => {
                format!(
                    "{BASE_API_URL}/me/time_entries?start_date={start_date}&end_date={end_date}"
                )
            }
            None => format!("{BASE_API_URL}/me/time_entries"),
        };

        self.c
            .get(url)
            .basic_auth(&self.token, Some("api_token"))
            .send()?
            .error_for_status()?
            .json::<Vec<TimeEntry>>()
    }

    pub fn get_current_entry(&self) -> Result<TimeEntry, reqwest::Error> {
        self.c
            .get(format!("{BASE_API_URL}/me/time_entries/current"))
            .basic_auth(&self.token, Some("api_token"))
            .send()?
            .error_for_status()?
            .json()
    }

    pub fn create_time_entry(&self, entry: NewTimeEntry) -> Result<TimeEntry, reqwest::Error> {
        let url = format!(
            "{BASE_API_URL}/workspaces/{}/time_entries",
            entry.workspace_id
        );

        self.c
            .post(url)
            .json(&entry)
            .basic_auth(&self.token, Some("api_token"))
            .send()?
            .error_for_status()?
            .json()
    }

    pub fn stop_time_entry(
        &self,
        workspace_id: &Number,
        time_entry_id: &Number,
    ) -> Result<TimeEntry, reqwest::Error> {
        let url =
            format!("{BASE_API_URL}/workspaces/{workspace_id}/time_entries/{time_entry_id}/stop");

        self.c
            .patch(url)
            .basic_auth(&self.token, Some("api_token"))
            .send()?
            .error_for_status()?
            .json()
    }

    pub fn get_projects(&self, workspace_id: &Number) -> Result<Vec<Project>, reqwest::Error> {
        self.c
            .get(format!("{BASE_API_URL}/workspaces/{workspace_id}/projects"))
            .basic_auth(&self.token, Some("api_token"))
            .send()?
            .error_for_status()?
            .json()
    }

    pub fn get_workspaces(&self) -> Result<Vec<Workspace>, reqwest::Error> {
        self.c
            .get(format!("{BASE_API_URL}/workspaces"))
            .basic_auth(&self.token, Some("api_token"))
            .send()?
            .error_for_status()?
            .json()
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

#[derive(Serialize, Debug)]
pub struct NewTimeEntry {
    pub created_with: String,
    pub description: Option<String>,
    pub duration: Number,
    pub project_id: Option<Number>,
    pub start: String,
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
