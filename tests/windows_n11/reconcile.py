"""Execute native harness discovery/cases, then reconcile the behavior evidence.

This is a blocking audit of real suites, not a substitute for them. The externally
maintained map cites observed cases; unsupported/missing mappings cannot become PASS.
"""
import argparse
import csv
import os
from pathlib import Path
import re
import tomllib

from evidence import ROOT, artifact, digest, load, main_failure, required, run, save


def inventory():
    items = {f"analysis:{path.name}" for path in (ROOT / "doc_internal/win_compat").glob("[0-9][0-9]-*.md")}
    items.add("crate:fish")
    items.update(f"crate:{path.parent.name}" for path in (ROOT / "crates").glob("*/Cargo.toml"))
    # Both disk and generated/embedded dispositions must be supplied by N9; the
    # package manifest alone cannot excuse omitted portable assets.
    for directory in ("share", "etc", "localization", "doc_src"):
        items.update(f"asset:{path.relative_to(ROOT).as_posix()}" for path in (ROOT / directory).rglob("*")
                     if path.is_file() and path.suffix not in (".pyc", ".pyo")
                     and not {"__pycache__", ".pytest_cache", "node_modules"}.intersection(path.parts))
    for directory, pattern in (("tests/checks", "*.fish"), ("tests/pexpects", "*.py")):
        items.update(f"unix-case:{path.relative_to(ROOT).as_posix()}" for path in (ROOT / directory).glob(pattern))
    text = (ROOT / "doc_internal/win_compat/coverage.md").read_text(encoding="utf-8")
    start = text.index("## Executable CLI option ledger")
    builtin_start = text.index("## Builtin option and subcommand ledger") + 3
    following = re.search(r"^## ", text[builtin_start:], re.MULTILINE)
    end = builtin_start + following.start() if following else len(text)
    ledger_lines = text[start:end].splitlines()
    for number, line in enumerate(ledger_lines):
        if number + 1 < len(ledger_lines) and ledger_lines[number + 1].startswith("|---"):
            continue
        if line.startswith("| "):
            surface = line.split("|", 2)[1].strip()
            items.add(f"surface:{surface}")
    items.update(f"native-module:{path.relative_to(ROOT).as_posix()}" for path in (ROOT / "src").rglob("*.rs")
                 if "windows" in path.relative_to(ROOT / "src").as_posix() or path.stem.startswith("native"))
    if len([key for key in items if key.startswith("analysis:")]) != 16:
        raise RuntimeError("original 01–16 component inventory is incomplete")
    return sorted(items)


def resources(evidence):
    rows = list(csv.DictReader((evidence / "resources.csv").open(encoding="utf-8")))
    warm = [row for row in rows if row["phase"] == "warmup"]
    settled = [row for row in rows if row["phase"] == "quiescent"]
    if len(warm) != 8 or len(settled) != 8 or len(rows) != 80:
        raise RuntimeError("sustained resource trace missing required phases/cycles")
    record = load(required("FISH_N11_RESOURCE_BUDGET"))
    required_fields = {"adopted_at", "adopted_before_revision", "source_baseline", "source_baseline_sha256", "host_context_sha256", "max_private_growth_bytes", "max_working_growth_bytes"}
    if not required_fields <= record.keys():
        raise RuntimeError("resource budgets require a recorded pre-adoption decision and source baseline identity")
    source = Path(record["source_baseline"]).resolve(strict=True)
    if digest(source) != record["source_baseline_sha256"]:
        raise RuntimeError("resource budget source baseline identity does not match recorded bytes")
    if record["host_context_sha256"] != digest(required("FISH_N11_HOST_CONTEXT")):
        raise RuntimeError("resource budget belongs to a different host/sink context")
    save(evidence / "resource-host-context.json", load(required("FISH_N11_HOST_CONTEXT")))
    (evidence / "resource-source-baseline").write_bytes(source.read_bytes())
    for field, budget in (("private_bytes", "max_private_growth_bytes"), ("working_bytes", "max_working_growth_bytes")):
        maximum = record[budget]
        if not isinstance(maximum, int) or maximum < 0:
            raise RuntimeError("resource budget must be a nonnegative pre-adopted byte value")
        growth = max(int(row[field]) for row in settled) - max(int(row[field]) for row in warm)
        if growth > maximum:
            raise RuntimeError(f"{field} growth {growth} exceeds pre-adopted {maximum}")
    save(evidence / "resource-budget-decision.json", record)


def execute(evidence, identity):
    spec = load(required("FISH_N11_SUITE_MANIFEST"))
    if spec.get("schema") != 1 or spec.get("artifact_sha256") != identity["sha256"]:
        raise RuntimeError("native suite manifest must identify this exact N10 ZIP")
    suites = spec.get("suites", [])
    configured = {suite["id"] for suite in suites}
    if len(configured) != len(suites) or "windows_n11" in configured:
        raise RuntimeError("suite IDs must be unique and must not recursively execute N11")
    save(evidence / "suite-manifest.json", spec)
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    mandatory = {target["name"] for target in cargo.get("test", []) if target["name"].startswith("windows_") and target["name"] != "windows_n11"}
    if not mandatory <= configured:
        raise RuntimeError(f"missing actual native harnesses: {sorted(mandatory - configured)}")
    if not any(suite.get("kind") == "root-library" for suite in suites):
        raise RuntimeError("root native library execution must be reconciled separately from integration suites")
    executed = set()
    reports = []
    for suite in suites:
        identifier = suite["id"]
        if not re.fullmatch(r"[A-Za-z0-9_-]+", identifier):
            raise RuntimeError("invalid suite evidence filename")
        binary = Path(suite["executable"]).resolve(strict=True)
        if suite.get("target") != "x86_64-pc-windows-msvc":
            raise RuntimeError(f"{identifier}: guest/MSYS results are not native execution")
        if suite.get("profile") not in ("debug", "release") or suite.get("features") not in ("default", "minimal"):
            raise RuntimeError(f"{identifier}: actual harness profile/features must be recorded")
        env = os.environ.copy()
        env.update(suite.get("env", {}))
        # The release leg of established N1–N7 fixtures is the installed package.
        # Their independently supplied debug leg remains explicitly distinct.
        env["FISH_NATIVE_TEST_RELEASE_BIN"] = str(required("FISH_N11_INSTALLED_BIN"))
        env["FISH_NATIVE_TEST_RELEASE_PROBE"] = str(required("FISH_N11_PROBE_BIN"))
        env["FISH_NATIVE_N6_RELEASE_KEY_READER"] = str(required("FISH_N11_INSTALLED_BIN").parent / "fish_key_reader.exe")
        if identifier in mandatory and not env.get("FISH_NATIVE_TEST_BIN"):
            raise RuntimeError(f"{identifier}: explicit debug artifact path is required; no misleading profile label")
        if identifier in mandatory and digest(env["FISH_NATIVE_TEST_BIN"]) == identity["installed_fish_sha256"]:
            raise RuntimeError(f"{identifier}: the same release image cannot be relabeled as a debug artifact")
        discovered = run([binary, "--list", "--format", "terse"], evidence, f"{identifier}-discovery", env=env).stdout.decode("utf-8")
        names = re.findall(r"^(.+): test$", discovered, re.MULTILINE)
        if not names or len(names) != len(set(names)):
            raise RuntimeError(f"{identifier}: empty or duplicate test discovery")
        if suite.get("expected_cases") != names:
            raise RuntimeError(f"{identifier}: discovery differs from the predeclared exact case registry: {names}")
        cases = []
        for number, name in enumerate(names):
            result = run([binary, "--exact", name, "--test-threads=1", "--nocapture", "--color", "never"], evidence, f"{identifier}-{number}", env=env, timeout=900)
            text = (result.stdout + result.stderr).decode("utf-8", errors="replace")
            if not re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", text):
                raise RuntimeError(f"{identifier}::{name}: mandatory case was ignored or not executed")
            case = f"{identifier}::{name}"
            executed.add(case)
            cases.append({"case": case, "family_records": re.findall(r"^PASS-CASE .+$", text, re.MULTILINE)})
        reports.append({"id": identifier, "binary": str(binary), "sha256": digest(binary),
                        "profile": suite["profile"], "features": suite["features"], "target": suite["target"],
                        "debug_artifact": env.get("FISH_NATIVE_TEST_BIN"),
                        "release_artifact": env["FISH_NATIVE_TEST_RELEASE_BIN"],
                        "debug_artifact_sha256": digest(env["FISH_NATIVE_TEST_BIN"]) if env.get("FISH_NATIVE_TEST_BIN") else None,
                        "release_artifact_sha256": digest(env["FISH_NATIVE_TEST_RELEASE_BIN"]),
                        "release_probe_sha256": digest(env["FISH_NATIVE_TEST_RELEASE_PROBE"]),
                        "discovered": len(names), "executed": len(cases), "ignored": 0, "cases": cases})
        save(evidence / "native-executions.json", reports)
    return executed


def reconcile(evidence):
    identity = artifact(evidence)
    discovered = sorted(set(inventory()) | {f"package:{entry['path']}" for entry in identity["manifest"]["files"]})
    save(evidence / "coverage-inventory.json", discovered)
    executed = execute(evidence, identity)
    local = evidence / "executed-cases.txt"
    if local.exists():
        executed.update("n11::" + name for name in local.read_text(encoding="utf-8").splitlines())
    mapping = load(required("FISH_N11_COVERAGE_MAP"))
    if mapping.get("artifact_sha256") != identity["sha256"]:
        raise RuntimeError("coverage map is not tied to this release ZIP")
    rows = mapping.get("items", {})
    missing = set(discovered) - rows.keys()
    stale = rows.keys() - set(discovered)
    problems = []
    for key in discovered:
        if key not in rows:
            continue
        row = rows[key]
        references = set(row.get("cases", []))
        failures = set(row.get("failure_cases", []))
        if not row.get("owner") or not row.get("behavior"):
            problems.append(f"{key}: no named implementation owner/behavior")
        if row.get("state") == "supported":
            if not references or not references <= executed or not failures or not failures <= executed:
                problems.append(f"{key}: supported behavior lacks executed positive/failure cases")
        elif row.get("state") in ("semantic-difference", "permanent-exclusion", "retained-unix"):
            if not row.get("reason") or not references or not references <= executed:
                problems.append(f"{key}: difference/Unix contract requires rationale and executed native alternate/rejection")
            if row["state"] == "retained-unix" and not key.startswith("unix-case:"):
                problems.append(f"{key}: a host-only label cannot excuse a runtime/portable component")
            if row["state"] == "permanent-exclusion" and row.get("contract") not in ("posix-fork", "posix-exec-same-pid", "posix-tty-fg-bg"):
                problems.append(f"{key}: only the three approved permanent POSIX exclusions may use this state")
            if row["state"] == "semantic-difference" and (row.get("preserves_portable_behavior") is not True or not row.get("decision_reference")):
                problems.append(f"{key}: semantic differences require an approved decision and preserved portable behavior, not an omission")
        else:
            problems.append(f"{key}: unresolved capability cannot close coverage")
        if row.get("optional_dependency"):
            if not set(row.get("present_cases", [])) <= executed or not row.get("present_cases") or not set(row.get("absent_cases", [])) <= executed or not row.get("absent_cases"):
                problems.append(f"{key}: missing executed present/absent optional dependency cases")
    report = {"artifact_sha256": identity["sha256"], "inventory": len(discovered), "executed_cases": sorted(executed),
              "missing": sorted(missing), "stale": sorted(stale), "problems": problems,
              "status": "pass" if not missing and not stale and not problems else "blocked"}
    save(evidence / "coverage-reconciliation.json", report)
    if missing or stale or problems:
        raise RuntimeError(f"coverage is unresolved: {len(missing)} missing, {len(stale)} stale, {len(problems)} invalid dispositions; see exact report")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--resources-only", action="store_true")
    args = parser.parse_args()
    def operation():
        if args.resources_only:
            artifact(args.evidence)
            resources(args.evidence)
        else:
            reconcile(args.evidence)
    main_failure(args.evidence, "N11-FAULTS" if args.resources_only else "N11-COVERAGE", operation)


if __name__ == "__main__":
    main()
