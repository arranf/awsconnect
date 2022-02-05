use std::cmp::Ordering;
use std::{convert::From};
use std::str::FromStr;

use strum::{EnumString, Display};


#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub name: String,
    pub arn: String,
    pub containers: Vec<Container>,
    pub status: TaskStatus
}


impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Task {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Task {
    pub fn friendly_output(&self) -> String {
        let mut containers = String::from("");
        let container_count = self.containers.len();
        for (index, container) in self.containers.iter().enumerate() {
            containers.push_str(container.pretty().as_str());
            if index != container_count - 1 {
                containers.push_str(", ");
            }
        }
        format!("{}{} ({}) [{}]", self.name, self.status.pretty_status(), self.arn, containers )
    }
}


impl From<rusoto_ecs::Task> for Task {
    fn from(item: rusoto_ecs::Task) -> Self {
        let name = item.task_definition_arn.as_ref().expect("Failed to get task arn from task").split(":task-definition/").nth(1).unwrap().split(":").nth(0).unwrap().to_owned();
        let containers = item.containers.expect("Failed to identify containers on task")
            .into_iter()
            .map(|c|Container { arn: c.container_arn.expect("Container had no ARN"), name: c.name.unwrap(), status: c.last_status.unwrap(), }).collect();
        Task { name, arn: item.task_arn.unwrap(), containers, status: TaskStatus::from_str(&item.last_status.unwrap()).unwrap()}
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    pub arn: String,
    pub name: String,
    pub status: String,
}


impl PartialOrd for Container {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Container {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl Container {
    fn pretty(&self) -> String {
        let status;
        if self.status == "RUNNING" {
            status = String::from("");
        } else {
            status = format!(" {}", self.status);
        }
        format!("{}{}", self.name, status)
    }
}

#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, EnumString, Display, Copy, Clone)]
pub enum TaskStatus {
    PROVISIONING,
    PENDING,
    ACTIVATING,
    RUNNING,
    DEACTIVATING,
    STOPPING,
    DEPROVISIONING,
    STOPPED
}

impl TaskStatus {
    fn pretty_status(self) -> String {
        if self != TaskStatus::RUNNING {
           return format!(" {}", self.to_string());
        }
        String::from("")
    }
}
