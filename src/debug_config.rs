use crate::defaults::{default_false, default_true};
use serde_derive::Deserialize;

/// Config file debug options
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ConfigFileDebugConfig {
    /// Print the name of the task before running
    #[serde(default = "default_true")]
    pub(crate) print_task_name: bool,
    /// Print the config file path when it is initialized
    #[serde(default = "default_false")]
    pub(crate) print_file_path: bool,
}

impl Default for ConfigFileDebugConfig {
    fn default() -> Self {
        Self {
            print_task_name: true,
            print_file_path: false,
        }
    }
}

/// Task debug options
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct TaskDebugConfig {
    /// Name of the task
    pub(crate) print_task_name: Option<bool>,
}

impl Clone for TaskDebugConfig {
    fn clone(&self) -> Self {
        Self {
            print_task_name: self.print_task_name,
        }
    }
}

pub(crate) struct ConcreteTaskDebugConfig {
    pub(crate) print_task_name: bool,
}

impl ConcreteTaskDebugConfig {
    pub(crate) fn new(
        task_debug_config: &Option<TaskDebugConfig>,
        config_file_debug_config: &ConfigFileDebugConfig,
    ) -> ConcreteTaskDebugConfig {
        let task_debug_config = match task_debug_config {
            Some(task_debug_config) => task_debug_config.clone(),
            None => TaskDebugConfig::default(),
        };

        ConcreteTaskDebugConfig {
            print_task_name: task_debug_config
                .print_task_name
                .unwrap_or(config_file_debug_config.print_task_name),
        }
    }
}
