use dialoguer::Confirm;
use std::env;
use tgl_cli::api;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("TOGGL_API_TOKEN").expect("missing TOGGL_API_TOKEN environment variable");
    let client = api::Client::new(token)?;
    let workspaces = client.get_workspaces()?;

    if Confirm::new().with_prompt("Print workspaces?").interact()? {
        println!("{workspaces:#?}");
    }

    for w in workspaces {
        let projects = client.get_projects(&w.id)?;
        let projects: Vec<_> = projects.iter().filter(|p| p.active).collect();

        if Confirm::new()
            .with_prompt("Print active projects?")
            .interact()?
        {
            println!("{projects:#?}");
        }

        let time_entries = client.get_time_entries(None)?;

        if Confirm::new()
            .with_prompt("Print recent time entries?")
            .interact()?
        {
            println!("{time_entries:#?}");
        }
    }

    Ok(())
}
