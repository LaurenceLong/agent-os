use crate::common;
use std::collections::BTreeSet;

#[test]
fn kernel_core_tool_surface_matches_current_model_visible_contract() {
    let state = common::Kernel::new().state_snapshot().unwrap();
    let names = state
        .tool_descriptors
        .values()
        .map(|descriptor| descriptor.name.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(
        names,
        BTreeSet::from([
            "accomplish_goal",
            "agent_control",
            "apply_patch",
            "ask_human",
            "glob_files",
            "grep_files",
            "load_skill",
            "post_blackboard",
            "read_file",
            "read_image",
            "read_skill_resource",
            "record_evidence",
            "report_supervisor",
            "request_permissions",
            "run_command",
            "set_goal",
            "submit_final",
            "tool_search",
            "update_checklist",
            "write_stdin",
        ])
    );

    let host_os_tools = names
        .intersection(&BTreeSet::from([
            "apply_patch",
            "glob_files",
            "grep_files",
            "read_file",
            "read_image",
            "run_command",
            "write_stdin",
        ]))
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        host_os_tools,
        BTreeSet::from([
            "apply_patch",
            "glob_files",
            "grep_files",
            "read_file",
            "read_image",
            "run_command",
            "write_stdin",
        ])
    );

    for legacy_name in [
        "find_files",
        "search_files",
        "workspace_discovery",
        "workspace_discover",
        "glob",
        "grep",
        "write_file",
        "replace_text",
        "delete_file",
    ] {
        assert!(
            !names.contains(legacy_name),
            "{legacy_name} must not become a parallel model-visible tool"
        );
    }
}
