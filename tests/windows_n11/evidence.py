"""Native evidence utilities; no shell command strings or shared-host cleanup."""
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import threading

ROOT = Path(__file__).resolve().parents[2]
_JOB = None


def owned_job():
    """Keep one noninheritable kill-on-close job until this runner exits.

    Only this runner and processes it subsequently creates enter the nested job.
    Never inspect, terminate, or reconfigure the host's enclosing Job Object.
    """
    global _JOB
    if os.name != "nt":
        raise RuntimeError("N11 evidence must execute native Windows processes")
    if _JOB is not None:
        return

    class Basic(ctypes.Structure):
        _fields_ = [("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64),
                    ("flags", ctypes.c_uint32), ("minimum", ctypes.c_size_t),
                    ("maximum", ctypes.c_size_t), ("active", ctypes.c_uint32),
                    ("affinity", ctypes.c_size_t), ("priority", ctypes.c_uint32),
                    ("scheduling", ctypes.c_uint32)]

    class Io(ctypes.Structure):
        _fields_ = [(name, ctypes.c_uint64) for name in
                    ("read_ops", "write_ops", "other_ops", "read", "write", "other")]

    class Limits(ctypes.Structure):
        _fields_ = [("basic", Basic), ("io", Io), ("process_memory", ctypes.c_size_t),
                    ("job_memory", ctypes.c_size_t), ("peak_process", ctypes.c_size_t),
                    ("peak_job", ctypes.c_size_t)]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
    kernel.CreateJobObjectW.restype = ctypes.c_void_p
    kernel.SetInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32]
    kernel.GetCurrentProcess.restype = ctypes.c_void_p
    kernel.AssignProcessToJobObject.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    handle = kernel.CreateJobObjectW(None, None)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    limits = Limits()
    limits.basic.flags = 0x2000
    if not kernel.SetInformationJobObject(handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
        raise ctypes.WinError(ctypes.get_last_error())
    if not kernel.AssignProcessToJobObject(handle, kernel.GetCurrentProcess()):
        raise ctypes.WinError(ctypes.get_last_error())
    # OS process teardown closes this handle after Python has flushed its evidence.
    _JOB = handle

    # A Windows venv launcher can be the process retained by the outer runner,
    # while this interpreter is its child. Keep a verified parent handle so an
    # outer timeout also terminates this interpreter and its own descendants.
    class FileTime(ctypes.Structure):
        _fields_ = [("low", ctypes.c_uint32), ("high", ctypes.c_uint32)]

    kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.GetProcessId.argtypes = [ctypes.c_void_p]
    kernel.GetProcessId.restype = ctypes.c_uint32
    kernel.GetProcessTimes.argtypes = [ctypes.c_void_p] + [ctypes.POINTER(FileTime)] * 4
    kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel.WaitForSingleObject.restype = ctypes.c_uint32
    kernel.TerminateJobObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    parent_pid = os.getppid()
    parent = kernel.OpenProcess(0x100000 | 0x1000, False, parent_pid)
    if not parent:
        raise ctypes.WinError(ctypes.get_last_error())

    def creation_time(process):
        times = [FileTime() for _ in range(4)]
        if not kernel.GetProcessTimes(process, *(ctypes.byref(value) for value in times)):
            raise ctypes.WinError(ctypes.get_last_error())
        return (times[0].high << 32) | times[0].low

    try:
        if (kernel.GetProcessId(parent) != parent_pid
                or creation_time(parent) > creation_time(kernel.GetCurrentProcess())):
            raise RuntimeError("owned runner parent identity changed before retention")
    except BaseException:
        kernel.CloseHandle(parent)
        raise

    def parent_lifetime():
        if kernel.WaitForSingleObject(parent, 0xFFFFFFFF) == 0:
            kernel.TerminateJobObject(handle, 1)
        # This also closes our kill-on-close handle if termination itself fails.
        os._exit(1)

    threading.Thread(target=parent_lifetime, name="fish-owned-parent-watch", daemon=True).start()


def required(name):
    value = os.environ.get(name)
    if not value:
        raise RuntimeError(f"BLOCKED: required fixture {name} is absent")
    return Path(value).resolve(strict=True)


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8-sig"))


def digest(path):
    with Path(path).open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def artifact(evidence):
    archive = required("FISH_N10_ARCHIVE")
    manifest = load(evidence / "n10-manifest.json")
    if manifest.get("product") != "fish" or manifest.get("target") != "x86_64-pc-windows-msvc":
        raise RuntimeError("not an N10 native x64 fish package")
    installed = required("FISH_N11_INSTALLED_BIN")
    root = installed.parent.parent
    files = manifest["files"]
    if not files:
        raise RuntimeError("empty N10 package manifest")
    for entry in files:
        relative = Path(entry["path"])
        destination = (root / relative).resolve(strict=True)
        if not destination.is_relative_to(root.resolve()):
            raise RuntimeError("package path escapes installed root")
        if digest(destination) != entry["sha256"].lower():
            raise RuntimeError(f"installed package changed: {relative}")
    identity = {"archive": str(archive), "sha256": digest(archive),
                "manifest": manifest, "installed_fish_sha256": digest(installed),
                "build_inputs": load(evidence / "build-inputs.json")}
    save(evidence / "artifact-identity.json", identity)
    (evidence / "artifact-version.txt").write_text(manifest["version"] + "\n", encoding="utf-8")
    return identity


def run(command, evidence, label, *, env=None, input=None, timeout=600, cwd=None):
    """communicate drains both streams and closes stdin; nested job owns descendants."""
    owned_job()
    command = [str(argument) for argument in command]
    process = subprocess.Popen(command, stdin=subprocess.PIPE if input is not None else subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, cwd=cwd, shell=False)
    try:
        stdout, stderr = process.communicate(input=input, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        # Do not perform an unbounded second communicate while a descendant owns
        # a pipe writer. Persist the observed partial data/timeout, then terminate
        # only the nested job created by this runner (including the failed runner).
        (evidence / f"{label}.stdout").write_bytes(error.output or b"")
        (evidence / f"{label}.stderr").write_bytes(error.stderr or b"")
        save(evidence / f"{label}.command.json", {"argv": command, "status": "timeout",
                                                "timeout_seconds": timeout, "streams": "partial/unavailable"})
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.TerminateJobObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        if not kernel.TerminateJobObject(_JOB, 98):
            raise ctypes.WinError(ctypes.get_last_error()) from error
        os._exit(98)
    result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    (evidence / f"{label}.stdout").write_bytes(result.stdout)
    (evidence / f"{label}.stderr").write_bytes(result.stderr)
    save(evidence / f"{label}.command.json", {"argv": command, "status": result.returncode,
                                           "cwd": str(cwd) if cwd else None})
    if result.returncode:
        raise RuntimeError(f"{label}: status {result.returncode}; retained native streams in {evidence}")
    return result


def main_failure(evidence, gate, operation):
    try:
        owned_job()
        operation()
    except Exception as error:
        save(evidence / f"{gate}-failure.json", {"status": "blocked-or-failed", "detail": str(error)})
        print(f"{gate}: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="Record and validate the exact installed N10 artifact before behavior runs")
    parser.add_argument("--evidence", type=Path, required=True)
    arguments = parser.parse_args()
    main_failure(arguments.evidence, "N11-ARTIFACT", lambda: artifact(arguments.evidence))
