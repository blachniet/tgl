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
            .get("https://api.track.toggl.com/api/v9/workspaces".to_string())
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