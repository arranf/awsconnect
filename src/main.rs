#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::single_match_else)]

use std::str::FromStr;
use std::{io::Read, env};

use log::{warn, debug};
use anyhow::{Result, Context, bail, anyhow};
use clap::StructOpt;
use dialoguer::{theme::ColorfulTheme, Select};
use aws_config::{profile::{Profile, load}};
use aws_types::os_shim_internal::{Env, Fs};
use rusoto_core::Region;
use rusoto_ecs::{Ecs, EcsClient, ListClustersRequest, ListTasksRequest, DescribeTasksRequest};
use subprocess::Exec;
use dotenv_parser::parse_dotenv;
use which::which;

mod cli;
mod task;

use crate::task::Container;
use crate::cli::Cli;
use crate::task::Task;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    confirm_dependencies()?;

    match cli.command {
        cli::Commands::Login {environment} => {
            let profile = get_profile(environment).await?;
            Exec::shell(format!("aws-vault login {}", profile.name())).join()?;
        }
        
        cli::Commands::Execute { environment, container, cluster, region, task } => {
            let profile = get_profile(environment).await?;
            setup_environment(&profile)?;

            let region = match region {
                Some(r) => Region::from_str(&r)?,
                None => Region::default(),
            };
            
            let ecs_client = EcsClient::new(region);
            let cluster_arn = get_cluster(cluster, &ecs_client).await?;
            let task = get_tasks(task, &cluster_arn, &ecs_client).await?;
            let container = choose_container(&task, container)?;
            execute_bash_container(&cluster_arn, &task, &container)?;
        },
    }

    Ok(())
}

/// Extracts the needed environment variables to call AWS commands from aws-vault, and adds them to the current process
fn setup_environment(profile: &Profile) -> Result<()> {
    let mut output = Exec::shell(format!("aws-vault exec {} -- env | grep AWS_", profile.name())).stream_stdout()?;
    let mut buffer = String::new();
    output.read_to_string(&mut buffer)?;
    let aws_environment_credentials = parse_dotenv(&buffer).expect("Failed to find AWS credentials");
    for (key, value) in &aws_environment_credentials {
        env::set_var(key, value);
    }
    Ok(())
}

/// Selects the current profile to use
async fn get_profile(passed_profile_name: Option<String>) -> Result<Profile> {
    let profile = load(&Fs::default(), &Env::default()).await?;
    debug!("Loaded AWS profiles");
    let profile_name = match passed_profile_name {
        Some(profile_name) => {
            debug!("Defaulting to profile option value: {profile_name}");
            profile_name
        } ,
        None => {
            let mut options = profile.profiles().filter(|p| *p != "default").collect::<Vec<_>>();
            options.sort_unstable();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Pick your environment")
                .default(0)
                .items(&options[..])
                .interact()
                .unwrap();

            let profile_name = options[selection].to_owned();
            debug!("Selected profile option value: {profile_name}");
            profile_name
        }
    };
    
    let profile = profile.get_profile(&profile_name).context("Couldn't find profile")?;
    Ok(profile.clone())
}

async fn get_cluster(cluster_name: Option<String>, client: &EcsClient) -> Result<String> {
    match cluster_name {
        Some(name) => Ok(name),
        None => {
            let result = client.list_clusters(ListClustersRequest::default()).await?;
            let mut clusters = result.cluster_arns.context("No clusters found")?;
            clusters.sort();
            let friendly_cluster_names: Vec<String> = clusters.iter().map(|name| name.clone().split(":cluster/").nth(1).unwrap().to_owned()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Pick your cluster")
                .default(0)
                .items(&friendly_cluster_names[..])
                .interact()
                .unwrap();
            
            Ok(clusters[selection].clone())
        }
    }
}

/// Gets all the running tasks across clusters the profile can access
async fn get_tasks(task: Option<String>, cluster: &str, client: &EcsClient) -> Result<Task> {
    match task {
        Some(name) => {
            let describe_request = DescribeTasksRequest { cluster: Some(String::from(cluster)), tasks: vec![name], ..DescribeTasksRequest::default() };
       
            let describe_result = client.describe_tasks(describe_request).await.context("Failed to contact ECS API and describe tasks")?;
            if describe_result.failures.as_ref().is_some() && !describe_result.failures.as_ref().unwrap().is_empty() {
                bail!("Failed to contact ESC API. Failed: {:?}", describe_result.failures.as_ref().unwrap());
            }
            let tasks = describe_result.tasks.context("No task found")?;
            Ok(Task::from( tasks.first().unwrap().clone()))
        },
        None => {
            let list_request = ListTasksRequest { cluster: Some(String::from(cluster)), ..ListTasksRequest::default() };

            let list_result = client.list_tasks(list_request).await.context("Failed to contact ECS API and list tasks")?;
            let task_arns = list_result.task_arns.context("No tasks found")?;

            let describe_request = DescribeTasksRequest { cluster: Some(String::from(cluster)), tasks: task_arns, ..DescribeTasksRequest::default()};
            let describe_result = client.describe_tasks(describe_request).await.context("Failed to contact ECS API and describe tasks")?;

            if describe_result.failures.as_ref().is_some() && !describe_result.failures.as_ref().unwrap().is_empty() {
                bail!("Failed to contact ESC API. Failed: {:?}", describe_result.failures.as_ref().unwrap());
            }

            let tasks = describe_result.tasks.context("No tasks found")?;
            let mut tasks: Vec<Task> = tasks.into_iter().map(Task::from).collect();
            tasks.sort();
            let friendly_task_names: Vec<String> = tasks.iter().map(task::Task::friendly_output).collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Pick your task")
                .default(0)
                .items(&friendly_task_names[..])
                .interact()
                .unwrap();

            let task = tasks[selection].clone();
            
            Ok(task)
        }
    }
}

fn choose_container(task: &Task, container_name: Option<String>) -> Result<Container> {
    match container_name {
        Some(name) => {
            task.containers.clone().into_iter().find(|c| (c.name == name) || c.arn == name ).ok_or_else(|| anyhow!("No container found matching"))
        },
        None => {
            if task.containers.len() == 1 {
                let c = task.containers.first().unwrap();
                return Ok(c.clone());
            }

            let friendly_container_name: Vec<String> = task.containers.iter().map(|c| c.name.clone()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("Pick your container")
                .default(0)
                .items(&friendly_container_name[..])
                .interact()
                .unwrap();
            
            Ok(task.containers[selection].clone())
        }
    }
}

fn execute_bash_container(cluster_arn: &str, task: &Task, container: &Container) -> Result<()> {
    Exec::shell(format!("aws ecs execute-command --cluster {} --task {} --container {} --command \"/usr/bin/env bash\" --interactive", cluster_arn, task.arn, container.name)).join()?;
    Ok(())
}

fn confirm_dependencies() -> Result<()> {
    which("aws-vault").map_err(|_| anyhow!("Failed to find aws-vault. Is it installed and in your PATH?"))?;
    which("aws").map_err(|_| anyhow!("Failed to find the AWS CLI. Is it installed and in your PATH?"))?;
    Ok(())
}
