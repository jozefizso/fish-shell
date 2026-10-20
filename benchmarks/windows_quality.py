"""N11 measured release consumers. Record raw samples before deciding budgets.

Run from the native windows_n11 quality gate. N6 reader metrics preserve their
original ConPTY workload; supplemental pipe workloads never borrow its budgets.
"""
import argparse
import concurrent.futures
import csv
import math
import os
from pathlib import Path
import platform
import shutil
import statistics
import sys
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests/windows_n11"))
from evidence import ROOT, artifact, digest, load, main_failure, required, run, save


def quote(text):
    return "'" + str(text).replace("\\", "\\\\").replace("'", "\\'") + "'"


def environment(root):
    env = {"SystemRoot": os.environ["SystemRoot"], "PATH": "", "LANG": "C"}
    for key, directory in (("HOME", "home"), ("USERPROFILE", "home"),
                           ("XDG_CONFIG_HOME", "config"), ("XDG_DATA_HOME", "data"),
                           ("XDG_CACHE_HOME", "cache"), ("APPDATA", "config"),
                           ("LOCALAPPDATA", "data"), ("TEMP", "temp"), ("TMP", "temp")):
        path = root / directory
        path.mkdir(parents=True, exist_ok=True)
        env[key] = str(path)
    return env


def summarize(values):
    ordered = sorted(values)
    return {"sample_count": len(values), "samples_ms": values,
            "median_ms": statistics.median(values),
            "nearest_rank_p95_ms": ordered[math.ceil(len(ordered) * .95) - 1]}


def adopt(metrics, baseline, names, problems, label):
    if not baseline.get("budgets", {}).get("adopted_before"):
        problems.append(f"{label}: no recorded pre-adopted budget decision")
        return
    for name in names:
        source = baseline.get("metrics", {}).get(name)
        if not source:
            problems.append(f"{label}: missing recorded metric {name}")
            continue
        for observed, budget in (("median_ms", "median_budget_ms"), ("nearest_rank_p95_ms", "p95_budget_ms")):
            limit = source.get(budget)
            if not isinstance(limit, (int, float)) or not math.isfinite(limit) or limit <= 0:
                problems.append(f"{label}/{name}: missing finite positive pre-adopted {budget}")
            elif metrics[name][observed] > limit:
                problems.append(f"{name}: {observed} {metrics[name][observed]:.4f} exceeds pre-adopted {limit:.4f}")
            metrics[name][budget] = limit


def signoffs(identity, problems):
    path = os.environ.get("FISH_N11_ACCESSIBILITY_SIGNOFFS")
    if not path:
        problems.append("BLOCKED: named human keyboard-only and screen-reader signoffs are absent")
        return None
    data = load(Path(path))
    required_observations = {"prompt", "edit", "pager", "unicode-navigation", "color-independent-feedback"}
    kinds = set()
    for entry in data.get("journeys", []):
        if entry.get("artifact_sha256") != identity["sha256"] or entry.get("result") != "pass":
            problems.append("accessibility record is not a passing observation of this exact artifact")
            continue
        if not entry.get("person", "").strip() or not entry.get("date"):
            problems.append("human signoff requires a named observer and actual observation date")
        terminal = entry.get("terminal", {})
        if not terminal.get("name") or not terminal.get("version"):
            problems.append("human journey lacks actual terminal name/version")
        if not required_observations <= entry.get("observations", {}).keys() or not all(entry["observations"].get(key) for key in required_observations):
            problems.append("human journey lacks actual prompt/edit/pager/Unicode/color-independent observations")
        if entry.get("kind") == "screen-reader":
            reader = entry.get("reader", {})
            if not reader.get("name") or not reader.get("version"):
                problems.append("screen-reader signoff lacks assistive-tool name/version")
        kinds.add(entry.get("kind"))
    if not {"keyboard-only", "screen-reader"} <= kinds:
        problems.append("both named keyboard-only and screen-reader human journeys are mandatory")
    return data


def measure(evidence, fish, probe):
    identity = artifact(evidence)
    context = load(required("FISH_N11_HOST_CONTEXT"))
    metadata = {"cpu", "os", "windows_build", "toolchain", "sdk", "profile", "features", "filesystem", "antivirus", "indexing", "inputs", "tools", "sink"}
    if not metadata <= context.keys() or any(context[key] is None or context[key] == "" for key in metadata):
        raise RuntimeError("benchmark context must record hardware/OS/build/profile/filesystem/protection/indexing/inputs/tools/sinks")
    if context["features"] != identity["manifest"]["features"] or context["profile"] != identity["build_inputs"]["configuration"]:
        raise RuntimeError("benchmark metadata cannot relabel the actual N10 artifact's profile/features")
    context["observed_platform"] = {"system": platform.system(), "version": platform.version(), "machine": platform.machine()}
    context["actual_artifact_sha256"] = identity["sha256"]
    context["probe_sha256"] = digest(probe)
    save(evidence / "benchmark-context.json", context)
    work = evidence / "benchmark-work"
    work.mkdir()
    metrics = {}
    raw = []
    problems = []
    clean_record = evidence / "daily-clean-machine.json"
    if not clean_record.is_file() or load(clean_record).get("externally_provisioned_clean_machine") is not True:
        problems.append("BLOCKED: daily keyboard behavior has no externally provisioned clean restricted-user machine attestation")
    reader = required("FISH_N11_READER_BENCH")
    # Preserve the existing driver's two-argument protocol and baseline workload.
    # Copy the entire actual installed package, not a developer image or selected
    # DLLs; adding the explicit test helper is not a runtime PATH dependency.
    copy = work / "reader-installed"
    shutil.copytree(fish.parent.parent, copy)
    shutil.copy2(probe, copy / "bin/windows_native_probe.exe")
    if digest(copy / "bin/fish.exe") != identity["installed_fish_sha256"]:
        raise RuntimeError("reader benchmark did not select the installed N10 image")
    result = run([reader, copy / "bin", work / "reader-sessions"], evidence, "reader-benchmark", timeout=600)
    reader_values = {}
    for row in csv.reader(result.stdout.decode("utf-8").splitlines()):
        if len(row) != 3:
            raise RuntimeError(f"unexpected reader measurement record: {row}")
        metric, sample, milliseconds = row
        value = float(milliseconds)
        if not math.isfinite(value) or value <= 0:
            raise RuntimeError("reader clock sample is not finite/positive")
        reader_values.setdefault(metric, []).append(value)
        raw.append({"metric": metric, "sample": int(sample), "warmup": False, "milliseconds": value, "sink": "rendered-owned-ConPTY/capture-file-ACK"})
    expected_reader = {"owned-console-first-prompt-ms": 16, "ascii-edit-capture-ack-ms": 32,
                       "unicode-edit-capture-ack-ms": 32, "native-child-submit-complete-ms": 16}
    if {name: len(samples) for name, samples in reader_values.items()} != expected_reader:
        raise RuntimeError("N6 reader workload discovery/sample count changed; no favorable substitute")
    metrics.update({name: summarize(values) for name, values in reader_values.items()})

    pipe = work / "pipe-workloads"
    pipe.mkdir()
    env = environment(pipe)
    globs = pipe / "glob"
    for directory in range(16):
        target = globs / f"directory-{directory:02}"
        target.mkdir(parents=True)
        for number in range(128):
            (target / f"snow-雪-{number:03}.txt").write_bytes(b"fixture\n")
    workloads = [
        ("batch-startup-drained-pipe-ms", "true", b""),
        ("native-child-drained-pipe-ms", f"{quote(probe)} exit 0", b""),
        ("large-pipeline-drained-pipe-ms", f"string repeat -n 1048576 x | {quote(probe)} n2-copy | string length", b"1048576\n"),
        ("glob-2048-drained-pipe-ms", "count glob/**/*.txt", b"2048\n"),
        ("completion-512-drained-pipe-ms", "complete -c bench -f -a " + quote(" ".join(f"choice{n:04}" for n in range(512))) + "; complete -C 'bench choice01' | count", b"100\n"),
    ]
    for metric, script, expected in workloads:
        values = []
        for sample in range(19):
            start = time.perf_counter_ns()
            output = run([fish, "-N", "-c", script], evidence, f"{metric}-{sample}", env=env, cwd=pipe, timeout=60)
            elapsed = (time.perf_counter_ns() - start) / 1_000_000
            if output.stdout != expected or output.stderr:
                raise RuntimeError(f"{metric}: actual behavior changed; expected exact bytes/status before accepting timing")
            raw.append({"metric": metric, "sample": sample - 3, "warmup": sample < 3, "milliseconds": elapsed, "sink": "concurrently-drained-stdout/stderr-pipes"})
            if sample >= 3:
                values.append(elapsed)
        metrics[metric] = summarize(values)
        save(evidence / "benchmark-raw.json", raw)

    merge_values = []
    for sample in range(19):
        name = f"quality_merge_{sample}"
        history = Path(env["XDG_DATA_HOME"]) / "fish" / f"{name}_history"
        history.parent.mkdir(exist_ok=True)
        history.write_text("".join(f"- cmd: seed-{n}\n  when: {1000 + n}\n" for n in range(1024)), encoding="utf-8")
        barrier = threading.Barrier(5)
        merge_env = dict(env, fish_history=name)
        def writer(number):
            barrier.wait(timeout=30)
            return run([fish, "-c", f"builtin history append merge-{sample}-{number}; builtin history save; or exit 91; builtin history merge; or exit 92; printf saved"], evidence, f"history-merge-{sample}-{number}", env=merge_env, cwd=pipe, timeout=60)
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
            workers = [pool.submit(writer, number) for number in range(4)]
            start = time.perf_counter_ns()
            barrier.wait(timeout=30)
            outputs = [worker.result() for worker in workers]
            elapsed = (time.perf_counter_ns() - start) / 1_000_000
        if any(output.stdout != b"saved" or output.stderr for output in outputs):
            raise RuntimeError("history benchmark did not acknowledge all real save/merge operations")
        verify = run([fish, "-c", "builtin history search | count"], evidence, f"history-verify-{sample}", env=merge_env, cwd=pipe)
        if verify.stdout != b"1028\n" or verify.stderr:
            raise RuntimeError("history benchmark lost a durable concurrent merge")
        raw.append({"metric": "history-1024-four-writer-merge-ms", "sample": sample - 3, "warmup": sample < 3, "milliseconds": elapsed, "sink": "four-concurrent-native-processes/drained-pipes/durable-history"})
        if sample >= 3:
            merge_values.append(elapsed)
        save(evidence / "benchmark-raw.json", raw)
    metrics["history-1024-four-writer-merge-ms"] = summarize(merge_values)
    save(evidence / "benchmark-workload-inventory.json", {
        "warmups": 3, "samples": 16, "reader_edit_samples": 32, "glob_files": 2048,
        "pipeline_input_bytes": 1048576, "history_seed_records": 1024, "history_concurrent_writers": 4,
        "completion_choices": 512, "pipe_scope": "process launch + actual workload + EOF, excluding fixture creation; includes evidence stream writes",
        "reader_scope": "unchanged benchmarks/windows_reader.rs; ConPTY bootstrap + rendered console/file barriers",
        "reader_driver_sha256": digest(reader), "probe_sha256": digest(probe)})

    n6_path = ROOT / "doc_internal/win_compat/baselines/N6.json"
    n6 = load(n6_path)
    equivalence = context.get("n6_baseline_equivalence", {})
    if equivalence.get("baseline_sha256") != digest(n6_path) or not all(equivalence.get(key) is True for key in ("same_host", "same_protection", "same_sink", "same_workload")):
        problems.append("BLOCKED: N6 baseline host/protection/sink/workload equivalence has not been recorded")
    elif context["profile"] != "release" or context["features"] != "minimal":
        problems.append("BLOCKED: recorded N6 baseline is minimal release, not this artifact profile/features")
    else:
        adopt(metrics, n6, expected_reader, problems, "N6")
    n1_path = ROOT / "doc_internal/win_compat/baselines/N1.json"
    supplementary = set(metrics) - expected_reader.keys()
    if not n1_path.is_file():
        problems.append("BLOCKED: recorded N1 baselines and pre-adopted numeric budgets are absent; raw supplemental workloads are measurements, not passing performance evidence")
    else:
        n1 = load(n1_path)
        n1_equivalence = context.get("n1_baseline_equivalence", {})
        if n1_equivalence.get("baseline_sha256") != digest(n1_path) or not all(n1_equivalence.get(key) is True for key in ("same_host", "same_protection", "same_sink", "same_workload")):
            problems.append("BLOCKED: N1 baseline host/protection/sink/workload equivalence is absent")
        else:
            adopt(metrics, n1, supplementary, problems, "N1")
    human = signoffs(identity, problems)
    if human:
        save(evidence / "human-signoffs.json", human)
    report = {"artifact_sha256": identity["sha256"], "metrics": metrics, "problems": problems,
              "status": "pass" if not problems else "blocked-or-failed",
              "accessibility_scope": "named external human records only; console capture is not screen-reader evidence"}
    save(evidence / "quality-result.json", report)
    # Remove only our copied benchmark fixtures; durable raw samples, commands,
    # streams, host identity and decisions remain available on either outcome.
    shutil.rmtree(work)
    if problems:
        raise RuntimeError("; ".join(problems))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", required=True, type=Path)
    parser.add_argument("fish", type=Path)
    parser.add_argument("probe", type=Path)
    args = parser.parse_args()
    def operation():
        try:
            measure(args.evidence, args.fish.resolve(strict=True), args.probe.resolve(strict=True))
        finally:
            owned = args.evidence / "benchmark-work"
            if owned.exists():
                shutil.rmtree(owned)
    main_failure(args.evidence, "N11-QUALITY", operation)


if __name__ == "__main__":
    main()
