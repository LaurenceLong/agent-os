#!/usr/bin/env python3
"""Private SWE-bench Lite 20-task runner helpers.

This module owns the benchmark artifact contract shared by Agent-OS and
OpenCode runs. The model runners may differ, but task prompts and SWE-bench
prediction rows must not.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Mapping, Sequence


@dataclass(frozen=True)
class BenchmarkTask:
    order: int
    instance_id: str
    repo: str
    base_commit: str
    fail_to_pass: tuple[str, ...]


@dataclass(frozen=True)
class ProviderSpec:
    base_url: str
    model: str
    api_key: str
    api_style: str


DEFAULT_SWEBENCH_VENV = Path("/root/agent-os-swebench-venv")
DEFAULT_SWEBENCH_DATASET = "SWE-bench/SWE-bench_Lite"
DEFAULT_SWEBENCH_SPLIT = "test"


def repository_root() -> Path:
    return Path(__file__).resolve().parents[2]


def parse_dotenv_content(content: str) -> dict[str, str]:
    values: dict[str, str] = {}
    for raw_line in content.splitlines():
        line = raw_line.strip().lstrip("\ufeff")
        if not line or line.startswith("#"):
            continue
        key, separator, value = line.partition("=")
        if not separator:
            continue
        key = key.strip().lstrip("\ufeff")
        if not key or key.startswith("#"):
            continue
        values[key] = normalize_dotenv_value(value.strip())
    return values


def normalize_dotenv_value(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    return value


def load_dotenv_values(dotenv_path: Path | None = None) -> dict[str, str]:
    path = repository_root() / ".env" if dotenv_path is None else dotenv_path
    if not path.exists():
        return {}
    return parse_dotenv_content(path.read_text(encoding="utf-8"))


def resolve_required_config_value(
    name: str,
    *,
    explicit: str | None = None,
    env: Mapping[str, str] | None = None,
    dotenv_values: Mapping[str, str] | None = None,
) -> str:
    if explicit is not None and explicit.strip():
        return explicit.strip()
    source_env = os.environ if env is None else env
    env_value = source_env.get(name, "").strip()
    if env_value:
        return env_value
    source_dotenv = load_dotenv_values() if dotenv_values is None else dotenv_values
    dotenv_value = source_dotenv.get(name, "").strip()
    if dotenv_value:
        return dotenv_value
    raise ValueError(f"{name} is required via CLI, process environment, or repository .env")


def resolve_config_value(
    name: str,
    *,
    explicit: str | None = None,
    env: Mapping[str, str] | None = None,
    dotenv_values: Mapping[str, str] | None = None,
    default: str,
) -> str:
    if explicit is not None and explicit.strip():
        return explicit.strip()
    source_env = os.environ if env is None else env
    env_value = source_env.get(name, "").strip()
    if env_value:
        return env_value
    source_dotenv = load_dotenv_values() if dotenv_values is None else dotenv_values
    dotenv_value = source_dotenv.get(name, "").strip()
    if dotenv_value:
        return dotenv_value
    return default


def load_manifest_tasks(manifest_path: Path) -> list[BenchmarkTask]:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    tasks = []
    for item in sorted(manifest["tasks"], key=lambda row: row["order"]):
        tasks.append(
            BenchmarkTask(
                order=int(item["order"]),
                instance_id=str(item["instance_id"]),
                repo=str(item["repo"]),
                base_commit=str(item["base_commit"]),
                fail_to_pass=tuple(str(test) for test in item.get("fail_to_pass", [])),
            )
        )
    return tasks


def build_task_prompt(task: BenchmarkTask, dataset_row: Mapping[str, object]) -> str:
    problem = str(dataset_row.get("problem_statement", "")).strip()
    fail_to_pass = "\n".join(f"- `{test}`" for test in task.fail_to_pass)
    if not fail_to_pass:
        fail_to_pass = "- No FAIL_TO_PASS tests listed in private manifest."

    return f"""# SWE-bench Lite Task

Instance: {task.instance_id}
Repo: {task.repo}
Base commit: {task.base_commit}

## Problem Statement

{problem}

## FAIL_TO_PASS Tests

{fail_to_pass}

## Benchmark Instructions

Solve the bug in this checked-out repository. Keep the change minimal and scoped
to the problem statement.

Use Agent-OS/OpenCode tools for reading, editing, and command evidence. Run the
most relevant failing test command if the local environment allows it. If the
test command cannot run because dependencies or platform tools are missing,
capture the exact command and error.

FAIL_TO_PASS names may refer to hidden or locally absent tests. If a listed test
or class is not present after one focused search, stop searching for that test
name and solve from the problem statement plus the production code path.

Do not inspect git history, previous commits, prior patches, or external sources
unless the problem statement explicitly asks for them. Solve from the current
checked-out source tree, the problem statement, and relevant tests.

Keep investigation bounded. Once the relevant code path is identified, make the
smallest scoped edit. After a focused fix, relevant validation or a captured
environment blocker, and git diff inspection, submit the final answer
immediately instead of repeating checks.

After you have a patch, one focused validation result or concrete environment
blocker, and one git diff inspection, the next action must be submit_final. Do
not keep exploring, rerun unrelated tests, or start a second fix.

Before final submission, inspect git diff and summarize changed files and
validation commands. Do not claim a test passed without command evidence. Do
not include any gold patch, hidden hints, or test patch in the solution.
"""


def write_predictions_jsonl(
    *,
    tasks: Sequence[BenchmarkTask],
    patch_dir: Path,
    output_path: Path,
    model_name: str,
) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w", encoding="utf-8", newline="\n") as handle:
        for task in tasks:
            patch_path = patch_dir / f"{task.instance_id}.patch"
            patch = patch_path.read_text(encoding="utf-8") if patch_path.exists() else ""
            row = {
                "instance_id": task.instance_id,
                "model_name_or_path": model_name,
                "model_patch": patch,
            }
            handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def load_run_instance_ids(summary_path: Path) -> list[str]:
    records = json.loads(summary_path.read_text(encoding="utf-8"))
    if not isinstance(records, list):
        raise ValueError(f"run summary must be a JSON array: {summary_path}")
    instance_ids = []
    for index, record in enumerate(records):
        if not isinstance(record, dict):
            raise ValueError(f"run summary item {index} must be a JSON object")
        instance_id = record.get("instance_id")
        if not isinstance(instance_id, str) or not instance_id:
            raise ValueError(f"run summary item {index} is missing instance_id")
        instance_ids.append(instance_id)
    if not instance_ids:
        raise ValueError(f"run summary contains no instances: {summary_path}")
    if len(set(instance_ids)) != len(instance_ids):
        raise ValueError(f"run summary contains duplicate instance ids: {summary_path}")
    return instance_ids


def build_swebench_harness_command(
    *,
    swebench_venv: Path,
    dataset_name: str,
    split: str,
    predictions_path: Path,
    instance_ids: Sequence[str],
    max_workers: int,
    timeout: int,
    run_id: str,
    report_dir: Path,
) -> list[str]:
    if not instance_ids:
        raise ValueError("official SWE-bench evaluation requires at least one instance id")
    python_bin = swebench_venv / "bin" / "python"
    if not python_bin.exists():
        raise FileNotFoundError(f"missing SWE-bench venv python: {python_bin}")
    return [
        python_bin.as_posix(),
        "-m",
        "swebench.harness.run_evaluation",
        "--dataset_name",
        dataset_name,
        "--split",
        split,
        "--predictions_path",
        predictions_path.as_posix(),
        "--instance_ids",
        *instance_ids,
        "--max_workers",
        str(max_workers),
        "--timeout",
        str(timeout),
        "--run_id",
        run_id,
        "--report_dir",
        report_dir.as_posix(),
    ]


def swebench_report_name(*, model_name: str, run_id: str) -> str:
    return f"{model_name.replace('/', '__')}.{run_id}.json"


def find_swebench_report(
    *,
    cwd: Path,
    report_dir: Path,
    model_name: str,
    run_id: str,
) -> Path:
    name = swebench_report_name(model_name=model_name, run_id=run_id)
    candidates = [cwd / name]
    if report_dir != cwd:
        candidates.append(report_dir / name)
    for path in candidates:
        if path.exists():
            return path
    candidate_list = ", ".join(path.as_posix() for path in candidates)
    raise FileNotFoundError(f"official SWE-bench report was not written; checked: {candidate_list}")


def swebench_report_passed(*, report_path: Path, expected_instance_ids: Sequence[str]) -> bool:
    expected = set(expected_instance_ids)
    report = json.loads(report_path.read_text(encoding="utf-8"))
    resolved = set(str(value) for value in report.get("resolved_ids", []))
    submitted = set(str(value) for value in report.get("submitted_ids", []))
    completed = set(str(value) for value in report.get("completed_ids", []))
    return submitted == expected and completed == expected and resolved == expected


def evaluate_agent_os_run(
    *,
    tasks: Sequence[BenchmarkTask],
    output_root: Path,
    swebench_venv: Path,
    dataset_name: str,
    split: str,
    model_name: str,
    instance_ids: Sequence[str],
    max_workers: int,
    timeout: int,
    run_id: str,
    predictions_output: Path | None = None,
    report_dir: Path | None = None,
    executor=None,
) -> int:
    if executor is None:
        executor = run_command
    selected = select_tasks(tasks, instance_ids)
    output_root.mkdir(parents=True, exist_ok=True)
    predictions_path = predictions_output or output_root / "agent-os-evaluation-predictions.jsonl"
    official_report_dir = report_dir or output_root / "swebench-report-official"
    official_report_dir.mkdir(parents=True, exist_ok=True)
    write_predictions_jsonl(
        tasks=selected,
        patch_dir=output_root / "agent-os" / "patches",
        output_path=predictions_path,
        model_name=model_name,
    )
    command = build_swebench_harness_command(
        swebench_venv=swebench_venv,
        dataset_name=dataset_name,
        split=split,
        predictions_path=predictions_path,
        instance_ids=[task.instance_id for task in selected],
        max_workers=max_workers,
        timeout=timeout,
        run_id=run_id,
        report_dir=official_report_dir,
    )
    exit_code = executor(command, cwd=output_root, env=os.environ)
    report_path = find_swebench_report(
        cwd=output_root,
        report_dir=official_report_dir,
        model_name=model_name,
        run_id=run_id,
    )
    if exit_code != 0:
        return exit_code
    return 0 if swebench_report_passed(
        report_path=report_path,
        expected_instance_ids=[task.instance_id for task in selected],
    ) else 1


def write_agent_os_provider_config(*, config_home: Path, provider: ProviderSpec) -> Path:
    path = config_home / "agent-os" / "providers.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    data = {
        "default_provider": "default",
        "providers": {
            "default": {
                "api_key": provider.api_key,
                "base_url": provider.base_url,
                "model": provider.model,
                "api_style": provider.api_style,
            }
        },
    }
    path.write_text(json.dumps(data, indent=2), encoding="utf-8", newline="\n")
    return path


def resolve_api_key(
    *,
    api_key_file: Path | None,
    api_key_env: str,
    env: Mapping[str, str] | None = None,
    dotenv_values: Mapping[str, str] | None = None,
) -> str:
    if api_key_file is not None:
        key = api_key_file.read_text(encoding="utf-8").strip()
        if not key:
            raise ValueError(f"API key file is empty: {api_key_file}")
        return key
    return resolve_required_config_value(
        api_key_env,
        env=env,
        dotenv_values=dotenv_values,
    )


def build_task_process_env(
    base_env: Mapping[str, str] | None = None,
    *,
    extra: Mapping[str, str] | None = None,
    prepend_paths: Sequence[Path] | None = None,
) -> dict[str, str]:
    env = dict(os.environ if base_env is None else base_env)
    runner_virtualenv = env.pop("VIRTUAL_ENV", None)
    env.pop("PYTHONPATH", None)
    env.pop("PYTHONHOME", None)
    env["PYTHONNOUSERSITE"] = "1"

    if runner_virtualenv and env.get("PATH"):
        venv_paths = {
            os.path.normcase(os.path.normpath(str(Path(runner_virtualenv) / "bin"))),
            os.path.normcase(os.path.normpath(str(Path(runner_virtualenv) / "Scripts"))),
        }
        path_entries = [
            entry
            for entry in env["PATH"].split(os.pathsep)
            if os.path.normcase(os.path.normpath(entry)) not in venv_paths
        ]
        env["PATH"] = os.pathsep.join(path_entries)

    if extra:
        env.update(extra)
    if prepend_paths:
        existing_path = env.get("PATH", "")
        leading = [path.as_posix() for path in prepend_paths]
        env["PATH"] = os.pathsep.join([*leading, existing_path] if existing_path else leading)
    return env


def prepare_agent_os_tool_bin(output_root: Path) -> Path:
    tool_bin = output_root / "agent-os" / "tool-bin"
    tool_bin.mkdir(parents=True, exist_ok=True)
    if os.name == "nt":
        shim = tool_bin / "python.cmd"
        shim.write_text("@echo off\r\npython3 %*\r\n", encoding="utf-8", newline="\r\n")
    else:
        shim = tool_bin / "python"
        shim.write_text("#!/usr/bin/env sh\nexec python3 \"$@\"\n", encoding="utf-8", newline="\n")
        shim.chmod(0o755)
    return tool_bin


def build_workspace_pythonpath(workspace: Path) -> str:
    entries = [workspace]
    for child in ("lib", "src"):
        path = workspace / child
        if path.exists():
            entries.append(path)
    return os.pathsep.join(path.as_posix() for path in entries)


def build_agent_os_command(
    *,
    agent_os_bin: Path,
    workspace: Path,
    state_db: Path,
    bundle_output: Path,
    task_file: Path,
    max_steps: int,
    runtime_timeout_seconds: int,
) -> list[str]:
    return [
        agent_os_bin.as_posix(),
        "chat",
        "--provider",
        "default",
        "--workspace",
        workspace.as_posix(),
        "--state-db",
        state_db.as_posix(),
        "--bundle-output",
        bundle_output.as_posix(),
        "--max-steps",
        str(max_steps),
        "--runtime-timeout-seconds",
        str(runtime_timeout_seconds),
        "--task-file",
        task_file.as_posix(),
    ]


def build_opencode_command(
    *,
    opencode_bin: str,
    workspace: Path,
    prompt_file: Path,
    model: str,
) -> list[str]:
    return [
        opencode_bin,
        "run",
        "Solve the attached SWE-bench Lite task. Keep the patch minimal, run relevant tests, inspect git diff, and stop after the fix is complete.",
        "--dir",
        workspace.as_posix(),
        "--model",
        model,
        "--format",
        "json",
        "--dangerously-skip-permissions",
        "--file",
        prompt_file.as_posix(),
    ]


def run_command(args: Sequence[str], *, cwd: Path, env: Mapping[str, str] | None = None) -> int:
    completed = subprocess.run(args, cwd=str(cwd), env=dict(env) if env is not None else None)
    return int(completed.returncode)


Executor = Callable[[Sequence[str]], int]
TaskExecutor = Callable[[Sequence[str]], int]


def run_command_to_log(
    command: Sequence[str],
    *,
    cwd: Path,
    env: Mapping[str, str],
    log_path: Path,
    timeout_seconds: int | None = None,
) -> int:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("w", encoding="utf-8", newline="\n") as log:
        try:
            completed = subprocess.run(
                list(command),
                cwd=str(cwd),
                env=dict(env),
                text=True,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=timeout_seconds,
            )
        except subprocess.TimeoutExpired:
            log.write(f"\n[private20_runner] task timed out after {timeout_seconds} seconds\n")
            return 124
    return int(completed.returncode)


def reset_workspace(repo_cache: Path, workspace: Path, task: BenchmarkTask) -> None:
    source = repo_cache / f"{task.repo.replace('/', '__')}.git"
    if not source.exists():
        raise FileNotFoundError(f"missing cached repository: {source}")
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["git", "clone", str(source), str(workspace)],
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    subprocess.run(
        ["git", "checkout", "--force", task.base_commit],
        cwd=str(workspace),
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    subprocess.run(
        ["git", "clean", "-fdx"],
        cwd=str(workspace),
        check=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def write_workspace_patch(workspace: Path, output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    completed = subprocess.run(
        ["git", "diff", "--binary"],
        cwd=str(workspace),
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    )
    output_path.write_text(completed.stdout, encoding="utf-8", newline="\n")


def remove_agent_os_state(state_db: Path) -> None:
    for path in (state_db, Path(f"{state_db}-wal"), Path(f"{state_db}-shm")):
        path.unlink(missing_ok=True)


def task_paths(output_root: Path, agent_name: str, instance_id: str) -> dict[str, Path]:
    root = output_root / agent_name
    return {
        "workspace": root / "workspaces" / instance_id,
        "prompt": root / "prompts" / f"{instance_id}.md",
        "log": root / "logs" / f"{instance_id}.log",
        "patch": root / "patches" / f"{instance_id}.patch",
        "record": root / "records" / f"{instance_id}.json",
        "state_db": root / "state" / f"{instance_id}.sqlite",
        "config_home": root / "config",
    }


def read_existing_task_record(output_root: Path, agent_name: str, instance_id: str) -> dict[str, object] | None:
    record_path = task_paths(output_root, agent_name, instance_id)["record"]
    if not record_path.exists():
        return None
    return json.loads(record_path.read_text(encoding="utf-8"))


def run_agent_os_task(
    *,
    task: BenchmarkTask,
    dataset_row: Mapping[str, object],
    repo_cache: Path,
    output_root: Path,
    agent_os_bin: Path,
    provider: ProviderSpec,
    max_steps: int,
    task_timeout_seconds: int | None = None,
    executor=run_command_to_log,
) -> dict[str, object]:
    paths = task_paths(output_root, "agent-os", task.instance_id)
    remove_agent_os_state(paths["state_db"])
    reset_workspace(repo_cache, paths["workspace"], task)
    prompt = build_task_prompt(task, dataset_row)
    paths["prompt"].parent.mkdir(parents=True, exist_ok=True)
    paths["prompt"].write_text(prompt, encoding="utf-8", newline="\n")
    tool_bin = prepare_agent_os_tool_bin(output_root)
    command = build_agent_os_command(
        agent_os_bin=agent_os_bin,
        workspace=paths["workspace"],
        state_db=paths["state_db"],
        bundle_output=Path("agent-os-task-bundle.json"),
        task_file=paths["prompt"],
        max_steps=max_steps,
        runtime_timeout_seconds=task_timeout_seconds or 3600,
    )
    paths["log"].parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=f"agent-os-provider-{task.instance_id}-") as config_tmp:
        config_home = Path(config_tmp)
        write_agent_os_provider_config(config_home=config_home, provider=provider)
        env = build_task_process_env(
            extra={
                "XDG_CONFIG_HOME": config_home.as_posix(),
                "PYTHONPATH": build_workspace_pythonpath(paths["workspace"]),
            },
            prepend_paths=[tool_bin],
        )
        exit_code = executor(
            command,
            cwd=paths["workspace"],
            env=env,
            log_path=paths["log"],
            timeout_seconds=task_timeout_seconds,
        )
    write_workspace_patch(paths["workspace"], paths["patch"])
    record = {
        "agent": "agent-os",
        "instance_id": task.instance_id,
        "exit_code": exit_code,
        "workspace": paths["workspace"].as_posix(),
        "prompt_path": paths["prompt"].as_posix(),
        "log_path": paths["log"].as_posix(),
        "patch_path": paths["patch"].as_posix(),
        "state_db": paths["state_db"].as_posix(),
        "task_timeout_seconds": task_timeout_seconds,
        "command": list(command),
    }
    paths["record"].parent.mkdir(parents=True, exist_ok=True)
    paths["record"].write_text(json.dumps(record, indent=2), encoding="utf-8", newline="\n")
    return record


def run_opencode_task(
    *,
    task: BenchmarkTask,
    dataset_row: Mapping[str, object],
    repo_cache: Path,
    output_root: Path,
    opencode_bin: str,
    model: str,
    task_timeout_seconds: int | None = None,
    executor=run_command_to_log,
) -> dict[str, object]:
    paths = task_paths(output_root, "opencode", task.instance_id)
    reset_workspace(repo_cache, paths["workspace"], task)
    prompt = build_task_prompt(task, dataset_row)
    paths["prompt"].parent.mkdir(parents=True, exist_ok=True)
    paths["prompt"].write_text(prompt, encoding="utf-8", newline="\n")

    env = build_task_process_env(extra={"PYTHONPATH": build_workspace_pythonpath(paths["workspace"])})
    command = build_opencode_command(
        opencode_bin=opencode_bin,
        workspace=paths["workspace"],
        prompt_file=paths["prompt"],
        model=model,
    )
    paths["log"].parent.mkdir(parents=True, exist_ok=True)
    exit_code = executor(
        command,
        cwd=paths["workspace"],
        env=env,
        log_path=paths["log"],
        timeout_seconds=task_timeout_seconds,
    )
    write_workspace_patch(paths["workspace"], paths["patch"])
    record = {
        "agent": "opencode",
        "instance_id": task.instance_id,
        "exit_code": exit_code,
        "workspace": paths["workspace"].as_posix(),
        "prompt_path": paths["prompt"].as_posix(),
        "log_path": paths["log"].as_posix(),
        "patch_path": paths["patch"].as_posix(),
        "task_timeout_seconds": task_timeout_seconds,
        "command": list(command),
    }
    paths["record"].parent.mkdir(parents=True, exist_ok=True)
    paths["record"].write_text(json.dumps(record, indent=2), encoding="utf-8", newline="\n")
    return record


def load_dataset_rows(dataset_name: str, split: str, tasks: Sequence[BenchmarkTask]) -> dict[str, Mapping[str, object]]:
    from datasets import load_dataset

    wanted = {task.instance_id for task in tasks}
    rows = {}
    for row in load_dataset(dataset_name, split=split):
        instance_id = row["instance_id"]
        if instance_id in wanted:
            rows[instance_id] = row
    missing = wanted - set(rows)
    if missing:
        raise ValueError(f"dataset is missing instances: {', '.join(sorted(missing))}")
    return rows


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Private SWE-bench Lite 20 helper")
    parser.add_argument("--manifest", type=Path, default=Path("benchmarks/swe-bench-lite/private-20.json"))
    subcommands = parser.add_subparsers(dest="command", required=True)

    predictions = subcommands.add_parser("predictions", help="write SWE-bench predictions JSONL")
    predictions.add_argument("--patch-dir", type=Path, required=True)
    predictions.add_argument("--output", type=Path, required=True)
    predictions.add_argument("--model-name", required=True)
    predictions.add_argument("--instance-id", action="append", default=[])

    run_agent = subcommands.add_parser("run-agent-os", help="run Agent-OS on selected tasks")
    run_agent.add_argument("--output-root", type=Path, required=True)
    run_agent.add_argument("--repo-cache", type=Path, required=True)
    run_agent.add_argument("--agent-os-bin", type=Path, required=True)
    run_agent.add_argument("--base-url")
    run_agent.add_argument("--model")
    run_agent.add_argument("--api-key-file", type=Path)
    run_agent.add_argument("--api-key-env", default="LLM_API_KEY")
    run_agent.add_argument("--api-style")
    run_agent.add_argument("--max-steps", type=int, default=48)
    run_agent.add_argument("--task-timeout-seconds", type=int, default=3600)
    run_agent.add_argument("--resume-existing", action="store_true")
    run_agent.add_argument("--dataset-name", default=DEFAULT_SWEBENCH_DATASET)
    run_agent.add_argument("--split", default=DEFAULT_SWEBENCH_SPLIT)
    run_agent.add_argument("--instance-id", action="append", default=[])

    evaluate_agent = subcommands.add_parser(
        "evaluate-agent-os",
        help="score an Agent-OS run with the official SWE-bench harness",
    )
    evaluate_agent.add_argument("--output-root", type=Path, required=True)
    evaluate_agent.add_argument("--swebench-venv", type=Path, default=DEFAULT_SWEBENCH_VENV)
    evaluate_agent.add_argument("--dataset-name", default=DEFAULT_SWEBENCH_DATASET)
    evaluate_agent.add_argument("--split", default=DEFAULT_SWEBENCH_SPLIT)
    evaluate_agent.add_argument("--model-name", default="agent-os")
    evaluate_agent.add_argument("--instance-id", action="append", default=[])
    evaluate_agent.add_argument("--max-workers", type=int, default=2)
    evaluate_agent.add_argument("--timeout", type=int, default=1800)
    evaluate_agent.add_argument("--run-id")
    evaluate_agent.add_argument("--predictions-output", type=Path)
    evaluate_agent.add_argument("--report-dir", type=Path)

    run_opencode = subcommands.add_parser("run-opencode", help="run OpenCode on selected tasks")
    run_opencode.add_argument("--output-root", type=Path, required=True)
    run_opencode.add_argument("--repo-cache", type=Path, required=True)
    run_opencode.add_argument("--opencode-bin", default="opencode")
    run_opencode.add_argument("--model", required=True)
    run_opencode.add_argument("--task-timeout-seconds", type=int, default=3600)
    run_opencode.add_argument("--resume-existing", action="store_true")
    run_opencode.add_argument("--dataset-name", default=DEFAULT_SWEBENCH_DATASET)
    run_opencode.add_argument("--split", default=DEFAULT_SWEBENCH_SPLIT)
    run_opencode.add_argument("--instance-id", action="append", default=[])

    return parser.parse_args(argv)


def select_tasks(tasks: Iterable[BenchmarkTask], instance_ids: Sequence[str]) -> list[BenchmarkTask]:
    if not instance_ids:
        return list(tasks)
    wanted = set(instance_ids)
    selected = [task for task in tasks if task.instance_id in wanted]
    missing = wanted - {task.instance_id for task in selected}
    if missing:
        raise ValueError(f"unknown instance ids: {', '.join(sorted(missing))}")
    return selected


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    tasks = load_manifest_tasks(args.manifest)
    if args.command == "predictions":
        selected = select_tasks(tasks, args.instance_id)
        write_predictions_jsonl(
            tasks=selected,
            patch_dir=args.patch_dir,
            output_path=args.output,
            model_name=args.model_name,
        )
        return 0
    if args.command == "run-agent-os":
        selected = select_tasks(tasks, args.instance_id)
        dotenv_values = load_dotenv_values()
        api_key = resolve_api_key(
            api_key_file=args.api_key_file,
            api_key_env=args.api_key_env,
            dotenv_values=dotenv_values,
        )
        rows = load_dataset_rows(args.dataset_name, args.split, selected)
        provider = ProviderSpec(
            base_url=resolve_required_config_value(
                "LLM_BASE_URL",
                explicit=args.base_url,
                dotenv_values=dotenv_values,
            ),
            model=resolve_required_config_value(
                "LLM_MODEL",
                explicit=args.model,
                dotenv_values=dotenv_values,
            ),
            api_key=api_key,
            api_style=resolve_config_value(
                "LLM_API_STYLE",
                explicit=args.api_style,
                dotenv_values=dotenv_values,
                default="anthropic-compatible",
            ),
        )
        records = []
        for task in selected:
            if args.resume_existing:
                existing = read_existing_task_record(args.output_root, "agent-os", task.instance_id)
                if existing is not None:
                    records.append(existing)
                    continue
            records.append(run_agent_os_task(
                task=task,
                dataset_row=rows[task.instance_id],
                repo_cache=args.repo_cache,
                output_root=args.output_root,
                agent_os_bin=args.agent_os_bin,
                provider=provider,
                max_steps=args.max_steps,
                task_timeout_seconds=args.task_timeout_seconds,
            ))
        summary = args.output_root / "agent-os" / "summary.json"
        summary.write_text(json.dumps(records, indent=2), encoding="utf-8", newline="\n")
        return 0
    if args.command == "evaluate-agent-os":
        instance_ids = args.instance_id or load_run_instance_ids(args.output_root / "agent-os" / "summary.json")
        run_id = args.run_id or f"agent-os-{args.output_root.name}"
        return evaluate_agent_os_run(
            tasks=tasks,
            output_root=args.output_root,
            swebench_venv=args.swebench_venv,
            dataset_name=args.dataset_name,
            split=args.split,
            model_name=args.model_name,
            instance_ids=instance_ids,
            max_workers=args.max_workers,
            timeout=args.timeout,
            run_id=run_id,
            predictions_output=args.predictions_output,
            report_dir=args.report_dir,
        )
    if args.command == "run-opencode":
        selected = select_tasks(tasks, args.instance_id)
        rows = load_dataset_rows(args.dataset_name, args.split, selected)
        records = []
        for task in selected:
            if args.resume_existing:
                existing = read_existing_task_record(args.output_root, "opencode", task.instance_id)
                if existing is not None:
                    records.append(existing)
                    continue
            records.append(run_opencode_task(
                task=task,
                dataset_row=rows[task.instance_id],
                repo_cache=args.repo_cache,
                output_root=args.output_root,
                opencode_bin=args.opencode_bin,
                model=args.model,
                task_timeout_seconds=args.task_timeout_seconds,
            ))
        summary = args.output_root / "opencode" / "summary.json"
        summary.write_text(json.dumps(records, indent=2), encoding="utf-8", newline="\n")
        return 0
    raise AssertionError(f"unhandled command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
