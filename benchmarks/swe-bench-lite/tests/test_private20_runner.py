import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from private20_runner import (
    BenchmarkTask,
    ProviderSpec,
    agent_os_config_model_id,
    build_agent_os_command,
    build_opencode_command,
    build_swebench_harness_command,
    build_task_process_env,
    build_task_prompt,
    build_workspace_pythonpath,
    evaluate_agent_os_run,
    load_manifest_tasks,
    load_run_instance_ids,
    load_dotenv_values,
    parse_args,
    parse_dotenv_content,
    prepare_agent_os_tool_bin,
    read_existing_task_record,
    resolve_api_key,
    resolve_config_value,
    resolve_required_config_value,
    run_command_to_log,
    run_agent_os_task,
    run_opencode_task,
    swebench_report_passed,
    write_agent_os_provider_config,
    write_predictions_jsonl,
)


class Private20RunnerTests(unittest.TestCase):
    def make_cached_repo(self, root: Path) -> tuple[Path, str]:
        source = root / "source"
        source.mkdir()
        subprocess_run(["git", "init"], cwd=source)
        subprocess_run(["git", "config", "user.email", "test@example.com"], cwd=source)
        subprocess_run(["git", "config", "user.name", "Test User"], cwd=source)
        (source / "bug.txt").write_text("before\n", encoding="utf-8")
        subprocess_run(["git", "add", "bug.txt"], cwd=source)
        subprocess_run(["git", "commit", "-m", "seed"], cwd=source)
        commit = subprocess_run(["git", "rev-parse", "HEAD"], cwd=source, capture=True).strip()

        repo_cache = root / "repo-cache"
        repo_cache.mkdir()
        subprocess_run(["git", "clone", "--bare", str(source), str(repo_cache / "demo__repo.git")], cwd=root)
        return repo_cache, commit

    def test_load_manifest_preserves_order_and_required_fields(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            manifest = Path(tmp) / "private-20.json"
            manifest.write_text(
                json.dumps(
                    {
                        "tasks": [
                            {
                                "order": 7,
                                "instance_id": "demo__repo-1",
                                "repo": "demo/repo",
                                "base_commit": "abc123",
                                "fail_to_pass": ["tests/test_demo.py::test_bug"],
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            tasks = load_manifest_tasks(manifest)

        self.assertEqual(
            tasks,
            [
                BenchmarkTask(
                    order=7,
                    instance_id="demo__repo-1",
                    repo="demo/repo",
                    base_commit="abc123",
                    fail_to_pass=("tests/test_demo.py::test_bug",),
                )
            ],
        )

    def test_build_task_prompt_contains_contract_and_no_gold_patch(self):
        task = BenchmarkTask(
            order=2,
            instance_id="django__django-11099",
            repo="django/django",
            base_commit="d26b242",
            fail_to_pass=("auth_tests.test_validators.UsernameValidatorsTests",),
        )
        dataset_row = {
            "problem_statement": "UsernameValidator allows trailing newline.",
            "hints_text": "Use anchors.",
            "patch": "diff --git a/secret b/secret\n",
            "test_patch": "diff --git a/tests b/tests\n",
        }

        prompt = build_task_prompt(task, dataset_row)

        self.assertIn("Instance: django__django-11099", prompt)
        self.assertIn("Repo: django/django", prompt)
        self.assertIn("UsernameValidator allows trailing newline.", prompt)
        self.assertIn("auth_tests.test_validators.UsernameValidatorsTests", prompt)
        self.assertIn("Use Agent-OS/OpenCode tools", prompt)
        self.assertIn("hidden or locally absent tests", prompt)
        self.assertIn("after one focused search", prompt)
        self.assertIn("Do not inspect git history", prompt)
        self.assertIn("submit the final answer", prompt)
        self.assertIn("immediately instead of repeating checks", prompt)
        self.assertIn("the next action must be submit_final", prompt)
        self.assertNotIn("diff --git", prompt)
        self.assertNotIn("Use anchors.", prompt)

    def test_write_predictions_jsonl_uses_empty_patch_for_missing_patch_file(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            root = Path(tmp)
            patch_dir = root / "patches"
            patch_dir.mkdir()
            (patch_dir / "demo__repo-1.patch").write_text("diff --git a/a b/a\n", encoding="utf-8")
            out = root / "predictions.jsonl"
            tasks = [
                BenchmarkTask(1, "demo__repo-1", "demo/repo", "abc", ()),
                BenchmarkTask(2, "demo__repo-2", "demo/repo", "def", ()),
            ]

            write_predictions_jsonl(
                tasks=tasks,
                patch_dir=patch_dir,
                output_path=out,
                model_name="agent-os-qwen3.6-plus",
            )

            rows = [json.loads(line) for line in out.read_text(encoding="utf-8").splitlines()]

        self.assertEqual(
            rows,
            [
                {
                    "instance_id": "demo__repo-1",
                    "model_name_or_path": "agent-os-qwen3.6-plus",
                    "model_patch": "diff --git a/a b/a\n",
                },
                {
                    "instance_id": "demo__repo-2",
                    "model_name_or_path": "agent-os-qwen3.6-plus",
                    "model_patch": "",
                },
            ],
        )

    def test_load_run_instance_ids_reads_exact_completed_run_summary(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            summary = Path(tmp) / "summary.json"
            summary.write_text(
                json.dumps(
                    [
                        {"instance_id": "demo__repo-2", "exit_code": 0},
                        {"instance_id": "demo__repo-1", "exit_code": 0},
                    ]
                ),
                encoding="utf-8",
            )

            instance_ids = load_run_instance_ids(summary)

        self.assertEqual(instance_ids, ["demo__repo-2", "demo__repo-1"])

    def test_load_run_instance_ids_rejects_duplicate_ids(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            summary = Path(tmp) / "summary.json"
            summary.write_text(
                json.dumps(
                    [
                        {"instance_id": "demo__repo-1", "exit_code": 0},
                        {"instance_id": "demo__repo-1", "exit_code": 0},
                    ]
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "duplicate"):
                load_run_instance_ids(summary)

    def test_build_swebench_harness_command_uses_wsl_venv_and_exact_ids(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            root = Path(tmp)
            venv = root / "venv"
            python_bin = venv / "bin" / "python"
            python_bin.parent.mkdir(parents=True)
            python_bin.write_text("#!/usr/bin/env python\n", encoding="utf-8")

            command = build_swebench_harness_command(
                swebench_venv=venv,
                dataset_name="SWE-bench/SWE-bench_Lite",
                split="test",
                predictions_path=root / "predictions.jsonl",
                instance_ids=["demo__repo-2", "demo__repo-1"],
                max_workers=2,
                timeout=1800,
                run_id="demo-run",
                report_dir=root / "report",
            )

        self.assertEqual(command[0], python_bin.as_posix())
        self.assertEqual(command[1:3], ["-m", "swebench.harness.run_evaluation"])
        self.assertIn("--instance_ids", command)
        start = command.index("--instance_ids") + 1
        self.assertEqual(command[start:start + 2], ["demo__repo-2", "demo__repo-1"])
        self.assertIn("--report_dir", command)

    def test_swebench_report_passed_requires_all_expected_ids_resolved(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            report = Path(tmp) / "agent-os.demo-run.json"
            report.write_text(
                json.dumps(
                    {
                        "submitted_ids": ["demo__repo-1", "demo__repo-2"],
                        "completed_ids": ["demo__repo-1", "demo__repo-2"],
                        "resolved_ids": ["demo__repo-1"],
                    }
                ),
                encoding="utf-8",
            )

            passed = swebench_report_passed(
                report_path=report,
                expected_instance_ids=["demo__repo-1", "demo__repo-2"],
            )

        self.assertFalse(passed)

    def test_evaluate_agent_os_run_writes_exact_predictions_and_fails_unresolved_report(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            root = Path(tmp)
            venv = root / "venv"
            python_bin = venv / "bin" / "python"
            python_bin.parent.mkdir(parents=True)
            python_bin.write_text("#!/usr/bin/env python\n", encoding="utf-8")
            patch_dir = root / "out" / "agent-os" / "patches"
            patch_dir.mkdir(parents=True)
            (patch_dir / "demo__repo-2.patch").write_text("diff --git b\n", encoding="utf-8")
            (patch_dir / "demo__repo-1.patch").write_text("diff --git a\n", encoding="utf-8")
            tasks = [
                BenchmarkTask(1, "demo__repo-1", "demo/repo", "abc", ()),
                BenchmarkTask(2, "demo__repo-2", "demo/repo", "def", ()),
                BenchmarkTask(3, "demo__repo-3", "demo/repo", "ghi", ()),
            ]
            captured = {}

            def fake_executor(command, *, cwd, env):
                captured["command"] = command
                captured["cwd"] = cwd
                captured["env"] = env
                (Path(cwd) / "agent-os.demo-run.json").write_text(
                    json.dumps(
                        {
                            "submitted_ids": ["demo__repo-1", "demo__repo-2"],
                            "completed_ids": ["demo__repo-1", "demo__repo-2"],
                            "resolved_ids": ["demo__repo-1"],
                        }
                    ),
                    encoding="utf-8",
                )
                return 0

            exit_code = evaluate_agent_os_run(
                tasks=tasks,
                output_root=root / "out",
                swebench_venv=venv,
                dataset_name="SWE-bench/SWE-bench_Lite",
                split="test",
                model_name="agent-os",
                instance_ids=["demo__repo-2", "demo__repo-1"],
                max_workers=2,
                timeout=1800,
                run_id="demo-run",
                executor=fake_executor,
            )

            predictions = [
                json.loads(line)
                for line in (root / "out" / "agent-os-evaluation-predictions.jsonl")
                .read_text(encoding="utf-8")
                .splitlines()
            ]

        self.assertEqual(exit_code, 1)
        self.assertEqual(captured["command"][0], python_bin.as_posix())
        self.assertEqual(captured["cwd"], root / "out")
        self.assertEqual([row["instance_id"] for row in predictions], ["demo__repo-1", "demo__repo-2"])
        self.assertNotIn("demo__repo-3", {row["instance_id"] for row in predictions})

    def test_read_existing_task_record_returns_record_when_present(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            output_root = Path(tmp) / "out"
            record_path = output_root / "agent-os" / "records" / "demo__repo-1.json"
            record_path.parent.mkdir(parents=True)
            record_path.write_text(
                json.dumps({"agent": "agent-os", "instance_id": "demo__repo-1", "exit_code": 0}),
                encoding="utf-8",
            )

            record = read_existing_task_record(output_root, "agent-os", "demo__repo-1")
            missing = read_existing_task_record(output_root, "agent-os", "demo__repo-2")

        self.assertEqual(record["instance_id"], "demo__repo-1")
        self.assertIsNone(missing)

    def test_write_agent_os_provider_config_uses_xdg_shape(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            config_home = Path(tmp) / "xdg"
            path = write_agent_os_provider_config(
                config_home=config_home,
                provider=ProviderSpec(
                    base_url="http://model.example/anthropic",
                    model="tongyi/qwen3.6-plus",
                    api_key="test-key",
                    api_style="anthropic-compatible",
                ),
            )
            data = json.loads(path.read_text(encoding="utf-8"))

        self.assertEqual(path.name, "config.json")
        self.assertEqual(path.parent.name, "agent-os")
        self.assertEqual(data["model"], "default/tongyi_qwen3.6-plus")
        provider = data["provider"]["default"]
        self.assertEqual(provider["api_key"], "test-key")
        self.assertEqual(provider["api_style"], "anthropic-compatible")
        self.assertEqual(provider["options"]["base_url"], "http://model.example/anthropic")
        model = provider["models"]["tongyi_qwen3.6-plus"]
        self.assertEqual(model["name"], "tongyi/qwen3.6-plus")
        self.assertEqual(model["limit"], {"context": 128000, "output": 8192})

    def test_agent_os_config_model_id_rejects_empty_request_model(self):
        self.assertEqual(agent_os_config_model_id(" provider/model "), "provider_model")
        with self.assertRaisesRegex(ValueError, "must not be empty"):
            agent_os_config_model_id(" ")

    def test_resolve_api_key_prefers_file_over_environment(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            key_file = Path(tmp) / "key.txt"
            key_file.write_text("file-key\n", encoding="utf-8")
            env = {"LLM_API_KEY": "env-key"}

            key = resolve_api_key(api_key_file=key_file, api_key_env="LLM_API_KEY", env=env)

        self.assertEqual(key, "file-key")

    def test_resolve_api_key_uses_environment_when_file_is_absent(self):
        key = resolve_api_key(api_key_file=None, api_key_env="LLM_API_KEY", env={"LLM_API_KEY": "env-key"})

        self.assertEqual(key, "env-key")

    def test_parse_dotenv_content_handles_bom_comments_and_quotes(self):
        values = parse_dotenv_content('\ufeff# comment\nLLM_MODEL="provider/model"\nPLAIN=value\nEMPTY=\n')

        self.assertEqual(values["LLM_MODEL"], "provider/model")
        self.assertEqual(values["PLAIN"], "value")
        self.assertEqual(values["EMPTY"], "")

    def test_load_dotenv_values_reads_given_path(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            dotenv = Path(tmp) / ".env"
            dotenv.write_text("LLM_BASE_URL='http://model.example/v1'\n", encoding="utf-8")

            values = load_dotenv_values(dotenv)

        self.assertEqual(values["LLM_BASE_URL"], "http://model.example/v1")

    def test_resolve_required_config_value_precedence_and_missing_error(self):
        value = resolve_required_config_value(
            "LLM_MODEL",
            explicit="explicit-model",
            env={"LLM_MODEL": "env-model"},
            dotenv_values={"LLM_MODEL": "dotenv-model"},
        )
        self.assertEqual(value, "explicit-model")

        value = resolve_required_config_value(
            "LLM_MODEL",
            explicit=None,
            env={"LLM_MODEL": "env-model"},
            dotenv_values={"LLM_MODEL": "dotenv-model"},
        )
        self.assertEqual(value, "env-model")

        value = resolve_required_config_value(
            "LLM_MODEL",
            explicit=None,
            env={},
            dotenv_values={"LLM_MODEL": "dotenv-model"},
        )
        self.assertEqual(value, "dotenv-model")

        with self.assertRaisesRegex(ValueError, "LLM_MODEL"):
            resolve_required_config_value("LLM_MODEL", env={}, dotenv_values={})

    def test_resolve_config_value_uses_default_after_env_and_dotenv(self):
        value = resolve_config_value(
            "LLM_API_STYLE",
            explicit=None,
            env={},
            dotenv_values={"LLM_API_STYLE": "openai-compatible"},
            default="anthropic-compatible",
        )
        self.assertEqual(value, "openai-compatible")

        value = resolve_config_value(
            "LLM_API_STYLE",
            explicit=None,
            env={},
            dotenv_values={},
            default="anthropic-compatible",
        )
        self.assertEqual(value, "anthropic-compatible")

    def test_resolve_api_key_uses_dotenv_when_environment_is_absent(self):
        key = resolve_api_key(
            api_key_file=None,
            api_key_env="LLM_API_KEY",
            env={},
            dotenv_values={"LLM_API_KEY": "dotenv-key"},
        )

        self.assertEqual(key, "dotenv-key")

    def test_run_agent_os_cli_allows_provider_values_from_dotenv(self):
        args = parse_args(
            [
                "run-agent-os",
                "--output-root",
                "out",
                "--repo-cache",
                "repo-cache",
                "--agent-os-bin",
                "agent-os",
                "--instance-id",
                "demo__repo-1",
            ]
        )

        self.assertIsNone(args.base_url)
        self.assertIsNone(args.model)
        self.assertIsNone(args.api_style)

    def test_build_task_process_env_removes_runner_python_state(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            venv = Path(tmp) / "runner-venv"
            venv_bin = venv / ("Scripts" if os.name == "nt" else "bin")
            other_bin = Path(tmp) / "tool-bin"
            base_env = {
                "PATH": os.pathsep.join([str(venv_bin), str(other_bin)]),
                "VIRTUAL_ENV": str(venv),
                "PYTHONPATH": "/stale/workspace",
                "PYTHONHOME": "/stale/python",
                "OPENCODE_CONFIG": "keep",
            }

            env = build_task_process_env(base_env)

        self.assertNotIn("VIRTUAL_ENV", env)
        self.assertNotIn("PYTHONPATH", env)
        self.assertNotIn("PYTHONHOME", env)
        self.assertEqual(env["PATH"], str(other_bin))
        self.assertEqual(env["PYTHONNOUSERSITE"], "1")
        self.assertEqual(env["OPENCODE_CONFIG"], "keep")

    def test_build_task_process_env_prepends_benchmark_tool_bin(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            tool_bin = Path(tmp) / "tool-bin"
            env = build_task_process_env(
                {"PATH": os.pathsep.join(["/usr/bin", "/bin"])},
                prepend_paths=[tool_bin],
            )

        self.assertEqual(env["PATH"].split(os.pathsep)[0], tool_bin.as_posix())

    def test_prepare_agent_os_tool_bin_writes_python_shim(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            tool_bin = prepare_agent_os_tool_bin(Path(tmp))

            shim = tool_bin / ("python.cmd" if os.name == "nt" else "python")
            self.assertTrue(shim.exists())
            self.assertIn("python3", shim.read_text(encoding="utf-8"))

    def test_build_workspace_pythonpath_includes_common_source_layouts(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            workspace = Path(tmp) / "repo"
            (workspace / "lib").mkdir(parents=True)
            (workspace / "src").mkdir()

            pythonpath = build_workspace_pythonpath(workspace)

        self.assertEqual(
            pythonpath,
            os.pathsep.join(
                [
                    workspace.as_posix(),
                    (workspace / "lib").as_posix(),
                    (workspace / "src").as_posix(),
                ]
            ),
        )

    def test_build_agent_os_command_uses_chat_state_db_and_bundle(self):
        command = build_agent_os_command(
            agent_os_bin=Path("/repo/target/wsl2-linux/debug/agent-os"),
            workspace=Path("/work/django"),
            state_db=Path("/out/django.sqlite"),
            bundle_output=Path("agent-os-task-bundle.json"),
            task_file=Path("/out/prompts/django.md"),
            model="default/provider_model",
            max_steps=48,
            runtime_timeout_seconds=3600,
        )

        self.assertEqual(command[0], "/repo/target/wsl2-linux/debug/agent-os")
        self.assertIn("chat", command)
        self.assertIn("--model", command)
        self.assertIn("default/provider_model", command)
        self.assertNotIn("--provider", command)
        self.assertIn("--workspace", command)
        self.assertIn("/work/django", command)
        self.assertIn("--state-db", command)
        self.assertIn("/out/django.sqlite", command)
        self.assertIn("--bundle-output", command)
        self.assertIn("agent-os-task-bundle.json", command)
        self.assertIn("--max-steps", command)
        self.assertIn("48", command)
        self.assertIn("--runtime-timeout-seconds", command)
        self.assertIn("3600", command)
        self.assertIn("--task-file", command)
        self.assertIn("/out/prompts/django.md", command)
        self.assertNotIn("Solve this task", command)

    def test_build_opencode_command_attaches_prompt_file_and_model(self):
        command = build_opencode_command(
            opencode_bin="opencode",
            workspace=Path("/work/django"),
            prompt_file=Path("/work/django/SWE_BENCH_TASK.md"),
            model="provider-alias/qwen3.6-plus",
        )

        self.assertEqual(command[:2], ["opencode", "run"])
        self.assertIn("--dir", command)
        self.assertIn("/work/django", command)
        self.assertIn("--model", command)
        self.assertIn("provider-alias/qwen3.6-plus", command)
        self.assertIn("--format", command)
        self.assertIn("json", command)
        self.assertIn("--dangerously-skip-permissions", command)
        self.assertIn("--file", command)
        self.assertIn("/work/django/SWE_BENCH_TASK.md", command)

    def test_run_agent_os_task_writes_prompt_provider_log_patch_and_record(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            root = Path(tmp)
            repo_cache, commit = self.make_cached_repo(root)
            task = BenchmarkTask(1, "demo__repo-1", "demo/repo", commit, ("tests/test_bug.py::test_bug",))
            output_root = root / "out"
            stale_state = output_root / "agent-os" / "state" / "demo__repo-1.sqlite"
            stale_state.parent.mkdir(parents=True)
            stale_state.write_text("stale state", encoding="utf-8")
            stale_state.with_suffix(".sqlite-wal").write_text("stale wal", encoding="utf-8")
            stale_state.with_suffix(".sqlite-shm").write_text("stale shm", encoding="utf-8")
            captured = {}

            def fake_executor(command, *, cwd, env, log_path, timeout_seconds=None):
                captured["command"] = command
                captured["env"] = env
                captured["timeout_seconds"] = timeout_seconds
                captured["state_removed_before_run"] = not stale_state.exists()
                captured["wal_removed_before_run"] = not stale_state.with_suffix(".sqlite-wal").exists()
                captured["shm_removed_before_run"] = not stale_state.with_suffix(".sqlite-shm").exists()
                provider_config = Path(env["XDG_CONFIG_HOME"]) / "agent-os" / "config.json"
                captured["provider_config_during_run"] = provider_config.exists()
                captured["provider_config_path"] = provider_config
                captured["provider_config"] = json.loads(provider_config.read_text(encoding="utf-8"))
                (Path(cwd) / "bug.txt").write_text("after\n", encoding="utf-8")
                Path(log_path).write_text("fake agent-os log\n", encoding="utf-8")
                return 0

            record = run_agent_os_task(
                task=task,
                dataset_row={"problem_statement": "Fix the bug."},
                repo_cache=repo_cache,
                output_root=output_root,
                agent_os_bin=Path("/bin/agent-os"),
                provider=ProviderSpec(
                    base_url="http://model.example/anthropic",
                    model="tongyi/qwen3.6-plus",
                    api_key="test-key",
                    api_style="anthropic-compatible",
                ),
                max_steps=12,
                task_timeout_seconds=34,
                executor=fake_executor,
            )

            patch = Path(record["patch_path"]).read_text(encoding="utf-8")
            provider_config_removed = not captured["provider_config_path"].exists()

        self.assertEqual(record["exit_code"], 0)
        self.assertIn("-before", patch)
        self.assertIn("+after", patch)
        self.assertTrue(captured["provider_config_during_run"])
        self.assertTrue(provider_config_removed)
        self.assertEqual(captured["provider_config"]["model"], "default/tongyi_qwen3.6-plus")
        self.assertTrue(captured["state_removed_before_run"])
        self.assertTrue(captured["wal_removed_before_run"])
        self.assertTrue(captured["shm_removed_before_run"])
        self.assertNotIn("VIRTUAL_ENV", captured["env"])
        self.assertEqual(captured["env"]["PYTHONPATH"], record["workspace"])
        self.assertEqual(captured["env"]["PYTHONNOUSERSITE"], "1")
        self.assertIn("chat", captured["command"])
        self.assertIn("--model", captured["command"])
        self.assertIn("default/tongyi_qwen3.6-plus", captured["command"])
        self.assertIn("--max-steps", captured["command"])
        self.assertIn("12", captured["command"])
        self.assertEqual(captured["timeout_seconds"], 34)

    def test_run_opencode_task_writes_prompt_log_patch_and_record(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            root = Path(tmp)
            repo_cache, commit = self.make_cached_repo(root)
            task = BenchmarkTask(1, "demo__repo-1", "demo/repo", commit, ("tests/test_bug.py::test_bug",))
            captured = {}

            def fake_executor(command, *, cwd, env, log_path, timeout_seconds=None):
                captured["command"] = command
                captured["env"] = env
                captured["timeout_seconds"] = timeout_seconds
                (Path(cwd) / "bug.txt").write_text("after-opencode\n", encoding="utf-8")
                Path(log_path).write_text("fake opencode log\n", encoding="utf-8")
                return 0

            record = run_opencode_task(
                task=task,
                dataset_row={"problem_statement": "Fix the bug."},
                repo_cache=repo_cache,
                output_root=root / "out",
                opencode_bin="opencode",
                model="provider-alias/qwen3.6-plus",
                task_timeout_seconds=56,
                executor=fake_executor,
            )

            patch = Path(record["patch_path"]).read_text(encoding="utf-8")

        self.assertEqual(record["exit_code"], 0)
        self.assertIn("+after-opencode", patch)
        self.assertEqual(captured["command"][:2], ["opencode", "run"])
        self.assertNotIn("VIRTUAL_ENV", captured["env"])
        self.assertEqual(captured["env"]["PYTHONPATH"], record["workspace"])
        self.assertEqual(captured["env"]["PYTHONNOUSERSITE"], "1")
        self.assertIn("--model", captured["command"])
        self.assertIn("provider-alias/qwen3.6-plus", captured["command"])
        self.assertEqual(captured["timeout_seconds"], 56)

    def test_run_command_to_log_returns_124_on_timeout(self):
        with tempfile.TemporaryDirectory(ignore_cleanup_errors=True) as tmp:
            root = Path(tmp)
            log_path = root / "logs" / "timeout.log"

            exit_code = run_command_to_log(
                [sys.executable, "-c", "import time; time.sleep(5)"],
                cwd=root,
                env=os.environ,
                log_path=log_path,
                timeout_seconds=1,
            )

            log = log_path.read_text(encoding="utf-8")

        self.assertEqual(exit_code, 124)
        self.assertIn("task timed out after 1 seconds", log)


def subprocess_run(args, *, cwd: Path, capture: bool = False) -> str:
    import subprocess

    completed = subprocess.run(
        args,
        cwd=str(cwd),
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE if capture else subprocess.DEVNULL,
    )
    return completed.stdout if capture else ""


if __name__ == "__main__":
    unittest.main()
