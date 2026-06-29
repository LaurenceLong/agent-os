use crate::{ModelClient, ModelTurnRequest, ModelTurnResponse};
use agent_os_sys::*;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct ExternalProcessModelClient {
    program: PathBuf,
    args: Vec<String>,
}

impl ExternalProcessModelClient {
    pub fn new(program: impl Into<PathBuf>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }

    pub fn program(&self) -> &PathBuf {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

impl ModelClient for ExternalProcessModelClient {
    fn next(&mut self, request: &ModelTurnRequest) -> AgentOsResult<ModelTurnResponse> {
        let request_bytes = serde_json::to_vec(request)?;
        let mut child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AgentOsError::Validation(format!(
                    "spawn external model process {}: {error}",
                    self.program.to_string_lossy()
                ))
            })?;
        let mut stdin = child.stdin.take().ok_or_else(|| {
            AgentOsError::Validation("external model process stdin was not piped".to_string())
        })?;
        stdin.write_all(&request_bytes).map_err(|error| {
            AgentOsError::Validation(format!("write external model request: {error}"))
        })?;
        drop(stdin);

        let output = child.wait_with_output().map_err(|error| {
            AgentOsError::Validation(format!("wait for external model process: {error}"))
        })?;
        if !output.status.success() {
            return Err(AgentOsError::Validation(format!(
                "external model process exited with status {}: {}",
                output.status,
                stderr_text(&output.stderr)
            )));
        }
        serde_json::from_slice(&output.stdout).map_err(AgentOsError::from)
    }
}

fn stderr_text(stderr: &[u8]) -> String {
    const LIMIT: usize = 4096;
    let text = String::from_utf8_lossy(stderr);
    let mut truncated: String = text.chars().take(LIMIT).collect();
    if text.chars().count() > LIMIT {
        truncated.push_str("...");
    }
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_os_kernel::{Kernel, RegisterGoalInput, SpawnAgentInput, SpawnTaskInput};
    use serde_json::json;
    use std::env;
    use std::fs;

    #[test]
    fn external_process_client_reads_json_response() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-external-model-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(&workspace).unwrap();
        let source_path = workspace.join("external_model_success.rs");
        let model_program = workspace.join(format!(
            "external_model_success{}",
            std::env::consts::EXE_SUFFIX
        ));
        fs::write(
            &source_path,
            r##"
use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    assert!(input.contains("\"step_index\":7"));
    io::stdout().write_all(br#"{"actions":[{"type":"output_text","text":"external model response"}],"usage":{"input_tokens":3,"output_tokens":2,"cost":0.0}}"#).unwrap();
}
"##,
        )
        .unwrap();
        let output = Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(&model_program)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "rustc failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let kernel = Kernel::new();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "external-model-test".to_string(),
                created_by: "agent-os-thread-test".to_string(),
                title: "External model".to_string(),
                description: "Exercise external model protocol".to_string(),
                acceptance_criteria: vec!["external model returns one response".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id,
                parent_task_id: None,
                title: "Run one model turn".to_string(),
                description: "Run one model turn".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 10,
                risk_level: 1,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id,
                role_profile_id: "role_worker".to_string(),
                owner: "agent-os-thread-test".to_string(),
                goal: "Run one external model turn".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![workspace.to_string_lossy().to_string()],
            })
            .unwrap();

        let mut client = ExternalProcessModelClient::new(model_program, Vec::new());
        let response = client
            .next(&ModelTurnRequest {
                thread: agent,
                workspace_root: workspace.clone(),
                step_index: 7,
                context: crate::ModelContextProjection::default(),
            })
            .unwrap();
        assert_eq!(response.usage.input_tokens, 3);
        assert_eq!(
            json!(response.actions),
            json!([{"type":"output_text","text":"external model response"}])
        );
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn external_process_client_reports_nonzero_exit() {
        let workspace = env::temp_dir().join(format!(
            "agent-os-external-model-fail-{}-{}",
            std::process::id(),
            new_id("case_")
        ));
        fs::create_dir_all(&workspace).unwrap();
        let source_path = workspace.join("external_model_failure.rs");
        let model_program = workspace.join(format!(
            "external_model_failure{}",
            std::env::consts::EXE_SUFFIX
        ));
        fs::write(
            &source_path,
            r#"
fn main() {
    eprintln!("model backend unavailable");
    std::process::exit(17);
}
"#,
        )
        .unwrap();
        let output = Command::new("rustc")
            .arg(&source_path)
            .arg("-o")
            .arg(&model_program)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "rustc failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let kernel = Kernel::new();
        let goal = kernel
            .register_goal(RegisterGoalInput {
                namespace: "external-model-test".to_string(),
                created_by: "agent-os-thread-test".to_string(),
                title: "External model".to_string(),
                description: "Exercise external model failure".to_string(),
                acceptance_criteria: vec!["external model failure is reported".to_string()],
                constraints: Vec::new(),
                risk_level: 1,
                deadline: None,
            })
            .unwrap();
        let task = kernel
            .spawn_task(SpawnTaskInput {
                goal_id: goal.goal_id,
                parent_task_id: None,
                title: "Run one model turn".to_string(),
                description: "Run one model turn".to_string(),
                depends_on: Vec::new(),
                required_artifact_types: Vec::new(),
                required_evidence_types: Vec::new(),
                priority: 10,
                risk_level: 1,
            })
            .unwrap();
        let agent = kernel
            .spawn_agent(SpawnAgentInput {
                task_id: task.task_id,
                role_profile_id: "role_worker".to_string(),
                owner: "agent-os-thread-test".to_string(),
                goal: "Run one external model turn".to_string(),
                success_criteria: Vec::new(),
                failure_criteria: Vec::new(),
                parent_thread_id: None,
                workspace_roots: vec![workspace.to_string_lossy().to_string()],
            })
            .unwrap();

        let mut client = ExternalProcessModelClient::new(model_program, Vec::new());
        let err = client
            .next(&ModelTurnRequest {
                thread: agent,
                workspace_root: workspace.clone(),
                step_index: 0,
                context: crate::ModelContextProjection::default(),
            })
            .unwrap_err();
        assert!(
            matches!(err, AgentOsError::Validation(message) if message.contains("model backend unavailable"))
        );
        let _ = fs::remove_dir_all(workspace);
    }
}
