"""Native N9 consumers. Dependencies are required, never replaced by stand-ins."""
import bz2
import gzip
import http.client
import importlib.util
import json
import lzma
import os
from pathlib import Path
import queue
import re
import signal
import shutil
import socket
import subprocess
import sys
import threading
import time
import traceback
from urllib.parse import urlsplit

ROOT = Path(sys.argv[2])
FISH = Path(sys.argv[3])
SOURCE = Path(sys.argv[4])
PYTHON = Path(sys.executable)
WEB = SOURCE / "share/tools/web_config/webconfig.py"
MAN = SOURCE / "share/tools/create_manpage_completions.py"


def fish_quote(value):
    return "'" + str(value).replace("\\", "\\\\").replace("'", "\\'") + "'"


def fish(code, *, env=None, expected=0):
    result = subprocess.run([str(FISH), "-N", "-c", code], input=b"", capture_output=True, env=env, timeout=20)
    assert result.returncode == expected, (code, result.returncode, result.stdout, result.stderr)
    return result


def interpreter():
    env = dict(os.environ, PATH=str(PYTHON.parent))
    selected = fish("__fish_anypython", env=env).stdout.decode().splitlines()
    assert selected and Path(selected[0]).samefile(PYTHON), selected
    fish("__fish_anypython", env=dict(os.environ, PATH=""), expected=1)
    result = fish("fish_config browse", env=dict(os.environ, PATH=""), expected=1)
    assert b"Python 3.8" in result.stdout + result.stderr
    old = Path(os.environ["FISH_N9_OLD_PYTHON"])
    version = subprocess.check_output([str(old), "--version"], stderr=subprocess.STDOUT).decode()
    assert version.startswith("Python 3.7."), version
    result = subprocess.run([str(old), str(MAN), "--help"], capture_output=True, timeout=10)
    assert result.returncode != 0 and b"Python 3.8" in result.stderr
    result = subprocess.run([str(old), str(WEB)], capture_output=True, timeout=10)
    assert result.returncode != 0 and b"Python 3.8" in result.stderr
    fish("__fish_anypython", env=dict(os.environ, PATH=str(old.parent)), expected=1)


def pages():
    directory = ROOT / "native manual input 雪"
    directory.mkdir()
    page = ('.TH N9TOOL "1" "October 2026" "n9 1.0" "User Commands"\n'
            '.SH NAME\nn9tool \\- native fixture\n.SH DESCRIPTION\nA real manual.\n'
            '.TP\n\\fB\\-s, \\-\\-snow\\fP\nShow snow Unicode 雪\n.PP\nTrailing paragraph.\n').encode()
    paths = []
    for name, compress in [("plain.1", lambda data: data), ("gzpage.1.gz", gzip.compress), ("bzpage.1.bz2", bz2.compress), ("xzpage.1.xz", lzma.compress)]:
        path = directory / name
        path.write_bytes(compress(page))
        paths.append(path)
    return directory, paths


def manpages():
    directory, paths = pages()
    output = ROOT / "generated Unicode 🐟"
    result = subprocess.run([str(PYTHON), str(MAN), "--directory", str(output), *map(str, paths)], capture_output=True, timeout=15)
    assert result.returncode == 0, result.stderr
    assert len(list(output.glob("*.fish"))) == 4
    assert not list(output.glob("*.tmp"))
    for name in ("plain", "gzpage", "bzpage", "xzpage"):
        generated = output / (name + ".fish")
        assert "雪" in generated.read_text(encoding="utf-8")
        result = fish("source %s; complete -C %s" % (fish_quote(generated), fish_quote(name + " --")))
        assert b"--snow" in result.stdout, result.stdout
    # Native MANPATH uses semicolons and retains drive prefixes.
    manuals = ROOT / "manual root 雪"
    (manuals / "man1").mkdir(parents=True)
    (manuals / "man1/n9discovered.1").write_bytes(paths[0].read_bytes())
    env = dict(os.environ, MANPATH=str(manuals), XDG_CACHE_HOME=str(ROOT / "manual-cache"))
    result = subprocess.run([str(PYTHON), str(MAN), "--manpath"], env=env, capture_output=True, timeout=15)
    assert result.returncode == 0, result.stderr
    assert (ROOT / "manual-cache/fish/generated_completions/n9discovered.fish").is_file()
    for bad in [directory / "missing.1", directory / "broken.1.gz"]:
        if bad.name.startswith("broken"):
            bad.write_bytes(b"not a compressed page")
        result = subprocess.run([str(PYTHON), str(MAN), "--directory", str(output), str(bad)], capture_output=True, timeout=15)
        assert result.returncode != 0 and bad.name.encode() in result.stderr, result.stderr
    foreground = ROOT / "foreground completions"
    result = fish("fish_update_completions --directory=%s %s" % (fish_quote(foreground), fish_quote(paths[0])), env=dict(os.environ, PATH=str(PYTHON.parent)))
    assert (foreground / "plain.fish").is_file(), result.stderr


class DirectoryChanges:
    """Native filesystem event barrier; never sleep/poll for background work."""
    def __init__(self, directory):
        import ctypes
        self.ctypes = ctypes
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.kernel.FindFirstChangeNotificationW.argtypes = [ctypes.c_wchar_p, ctypes.c_int, ctypes.c_uint32]
        self.kernel.FindFirstChangeNotificationW.restype = ctypes.c_void_p
        self.kernel.FindNextChangeNotification.argtypes = [ctypes.c_void_p]
        self.kernel.FindCloseChangeNotification.argtypes = [ctypes.c_void_p]
        self.kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
        self.handle = self.kernel.FindFirstChangeNotificationW(str(directory), True, 0x1 | 0x2 | 0x10)
        assert self.handle not in (None, ctypes.c_void_p(-1).value), ctypes.WinError(ctypes.get_last_error())

    def until(self, predicate):
        deadline = time.monotonic() + 15
        while True:
            assert self.kernel.FindNextChangeNotification(self.handle)
            if predicate():
                return
            remaining = deadline - time.monotonic()
            assert remaining > 0, "native filesystem event deadline"
            assert self.kernel.WaitForSingleObject(self.handle, int(remaining * 1000)) == 0, "background generation did not complete"

    def close(self):
        assert self.kernel.FindCloseChangeNotification(self.handle)


def background():
    _, paths = pages()
    output = ROOT / "background completions"
    temporary = Path(os.environ["TEMP"])
    before = set(temporary.iterdir())
    changes = DirectoryChanges(ROOT)
    try:
        # The initiating shell exits immediately. The N5-disowned native Python
        # child owns its private script and must not retain shell pipe workers.
        code = "set -g fish_update_completions_detach true; fish_update_completions --directory=%s %s" % (fish_quote(output), fish_quote(paths[0]))
        fish(code, env=dict(os.environ, PATH=str(PYTHON.parent)))
        changes.until(lambda: (output / "plain.fish").is_file() and set(temporary.iterdir()) == before)
    finally:
        changes.close()
    result = fish("source %s; complete -C 'plain --'" % fish_quote(output / "plain.fish"))
    assert b"--snow" in result.stdout
    assert not list(output.glob("*.tmp"))


def module():
    spec = importlib.util.spec_from_file_location("fish_n9_webconfig", WEB)
    web = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(web)
    web.FISH_BIN_PATH = str(FISH)
    web.initial_wd = str(ROOT)
    web.authkey = os.urandom(16).hex()
    return web


def web_stage(name):
    # Persist the last completed boundary even when the outside process bound
    # terminates the interpreter before its captured streams can be reported.
    with (ROOT / "web-stages.txt").open("a", encoding="utf-8") as evidence:
        evidence.write("%.3f %s\n" % (time.monotonic(), name))


def request(server, path, data=None):
    connection = http.client.HTTPConnection("127.0.0.1", server.server_address[1], timeout=15)
    try:
        headers = {"Content-Type": "application/json"} if data is not None else {}
        connection.request("POST" if data is not None else "GET", path, json.dumps(data) if data is not None else None, headers)
        response = connection.getresponse()
        return response.status, response.read()
    finally:
        connection.close()


def remove_owned_browser_junction(case_root, profile_home):
    root = case_root.resolve(strict=True)
    home = profile_home.resolve(strict=True)
    home.relative_to(root)
    cache = home / "AppData/Local/Microsoft/Windows/INetCache"
    junction = cache / "Content.IE5"
    try:
        metadata = junction.lstat()
    except FileNotFoundError:
        return
    if not metadata.st_file_attributes & 0x400:
        return
    assert metadata.st_reparse_tag == 0xa0000003, "unexpected browser reparse type"
    expected = (cache / "IE").resolve(strict=True)
    expected.relative_to(root)
    assert junction.samefile(expected), "browser junction target is not its owned IE cache"
    # Remove only the known junction, never traverse or delete its target.
    junction.rmdir()


def web_edits():
    from playwright.sync_api import sync_playwright
    import ctypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.IsProcessInJob.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)]
    kernel.TerminateProcess.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel.WaitForSingleObject.restype = ctypes.c_uint32
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    web = module()
    rejected_peers = queue.Queue()

    class PeerObservedServer(web.FishConfigTCPServer):
        def verify_request(self, connection, address):
            accepted = super().verify_request(connection, address)
            if not accepted:
                rejected_peers.put(address[0])
            return accepted

    web.FishConfigTCPServer.address_family = socket.AF_INET
    old_cwd = os.getcwd()
    os.chdir(WEB.parent)
    server = PeerObservedServer(("127.0.0.1", 0), web.FishConfigHTTPRequestHandler)
    worker = threading.Thread(target=server.serve_forever, name="n9-live-http")
    worker.start()
    prefix = "/" + web.authkey
    frozen = Path(os.environ["XDG_CONFIG_HOME"]) / "fish/conf.d/fish_frozen_theme.fish"
    prompt = Path(os.environ["XDG_CONFIG_HOME"]) / "fish/functions/fish_prompt.fish"
    try:
        assert listener_pids("127.0.0.1", server.server_address[1]) == {os.getpid()}
        web_stage("authenticated static GET start")
        assert request(server, prefix + "/") == (200, (WEB.parent / "index.html").read_bytes())
        web_stage("authenticated static GET complete")
        edit = {"fish_prompt": "function fish_prompt; printf 'N9 web 雪> '; end"}
        assert request(server, "/set_prompt/", edit)[0] == 403
        assert request(server, "/invalid/set_prompt/", edit)[0] == 403
        assert request(server, prefix + "x/set_prompt/", edit)[0] == 403
        # Malformed, oversized and unfinished rejected bodies must remain
        # unauthenticated and cannot hold an HTTP worker indefinitely.
        for length in ("invalid", str(64 * 1024 + 1), "100"):
            connection = http.client.HTTPConnection("127.0.0.1", server.server_address[1], timeout=15)
            try:
                body = b"{" if length == "100" else b""
                connection.request("POST", "/invalid/set_prompt/", body,
                                   {"Content-Type": "application/json", "Content-Length": length})
                response = connection.getresponse()
                assert response.status == 403
                response.read()
            finally:
                connection.close()
        assert not prompt.exists()
        # Own one listener on an exact assigned nonloopback interface solely
        # for the peer-rejection fixture. Production still binds only loopback.
        local_address = nonloopback_ipv4_address()
        assert local_address not in web.FishConfigTCPServer.WHITELIST
        peer_server = PeerObservedServer((local_address, 0), web.FishConfigHTTPRequestHandler)
        peer_worker = threading.Thread(target=peer_server.serve_forever, name="n9-nonloopback-http")
        peer_worker.start()
        connection = http.client.HTTPConnection(local_address, peer_server.server_address[1], timeout=15,
                                               source_address=(local_address, 0))
        try:
            assert listener_pids(local_address, peer_server.server_address[1]) == {os.getpid()}
            try:
                connection.connect()
            except OSError as error:
                raise AssertionError(("required nonloopback IPv4 fixture unreachable",
                                      local_address, peer_server.server_address[1])) from error
            try:
                connection.request("POST", prefix + "/set_prompt/", json.dumps(edit),
                                   {"Content-Type": "application/json"})
                connection.getresponse()
            except (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError):
                pass
            else:
                raise AssertionError("non-whitelisted peer received an HTTP response")
            assert rejected_peers.get(timeout=5) == local_address
        finally:
            connection.close()
            peer_server.shutdown()
            peer_worker.join(timeout=5)
            peer_server.server_close()
        assert not peer_worker.is_alive()
        assert not listener_pids(local_address, peer_server.server_address[1]), "nonloopback fixture listener leaked"
        assert not prompt.exists() and not frozen.exists()
        web_stage("authentication and real nonloopback rejection complete")
        with sync_playwright() as playwright:
            executable = os.environ.get("FISH_N9_BROWSER")
            native_browser = Path(executable) if executable else Path(Path(os.environ["SystemRoot"]).anchor) / "Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
            assert native_browser.is_file(), "required native Edge/Chromium FISH_N9_BROWSER fixture"
            profile = ROOT / "owned native browser profile 雪"
            profile.mkdir()
            screenshot = ROOT / "native browser launch.png"
            redirect = ROOT / "native browser redirect.html"
            address = "http://127.0.0.1:%d%s/" % (server.server_address[1], prefix)
            redirect.write_text('<meta http-equiv="refresh" content="0;URL=' + address + '" />', encoding="utf-8")
            saved_browser = os.environ.get("BROWSER")
            os.environ["BROWSER"] = subprocess.list2cmdline([
                str(native_browser), "--headless=new", "--guest", "--no-first-run",
                "--no-default-browser-check", "--user-data-dir=" + str(profile), "%s",
            ])
            launched = browser = browser_process = None
            try:
                # Use the real browser's security defaults, not Playwright's
                # default --no-sandbox / disabled phishing and sign-in checks.
                # Guest mode also keeps Windows account auto-sign-in out of
                # this disposable profile; it does not override browser policy.
                context = playwright.chromium.launch_persistent_context(
                    str(profile), executable_path=str(native_browser), headless=True,
                    chromium_sandbox=True, ignore_default_args=True,
                    args=["--headless=new", "--guest", "--no-first-run",
                          "--no-default-browser-check", "--edge-skip-compat-layer-relaunch",
                          "--remote-debugging-pipe", "--user-data-dir=" + str(profile),
                          "about:blank"],
                    viewport={"width": 800, "height": 600}, timeout=10000)
                browser = context.browser
                assert browser is not None
                process_info = browser.new_browser_cdp_session().send("SystemInfo.getProcessInfo")
                browser_pids = [item["id"] for item in process_info["processInfo"] if item["type"] == "browser"]
                assert len(browser_pids) == 1, process_info
                browser_pid = browser_pids[0]
                browser_process = kernel.OpenProcess(0x100000 | 0x1000 | 1, False, browser_pid)
                assert browser_process, ctypes.WinError(ctypes.get_last_error())
                member = ctypes.c_int()
                assert kernel.IsProcessInJob(browser_process, ownership._JOB, ctypes.byref(member))
                assert member.value, "native browser is outside this probe's owned job"
                web_stage("native Edge pipe transport ready")
                # Exercise the actual BROWSER dispatch into that same instance.
                # Native singleton handoff must open a new real redirect page.
                with context.expect_page(timeout=15000) as opened:
                    launched = web.launch_browser(redirect.as_uri(), address)
                    assert isinstance(launched, subprocess.Popen)
                    assert launched.wait(timeout=5) == 0
                page = opened.value
                page.set_default_timeout(15000)
                web_stage("native browser singleton handoff complete")
                page.wait_for_url(address)
                web_stage("browser redirect complete")
                choice = page.locator(".color_scheme_choice_container").filter(has_text="Dracula").first
                choice.wait_for()
                web_stage("theme choices rendered")
                choice.click()
                page.get_by_role("button", name="Set Theme", exact=True).click()
                page.get_by_role("button", name="Theme Set!", exact=True).wait_for()
                assert frozen.is_file()
                selected = subprocess.run([str(FISH), "-c", "printf '%s' $fish_color_command"], input=b"", capture_output=True, timeout=15)
                assert selected.returncode == 0 and selected.stdout == b"8be9fd", (selected.stdout, selected.stderr)
                exported = subprocess.run([str(FISH), "-c", "fish_config theme dump"], input=b"", capture_output=True, timeout=15)
                assert exported.returncode == 0, exported.stderr
                theme_export = ROOT / "browser saved theme 雪.theme"
                theme_export.write_bytes(exported.stdout)
                imported = subprocess.run(
                    [str(FISH), "-c", "set -g fish_color_command red; fish_config theme choose %s; or exit; printf '%%s' $fish_color_command" % fish_quote(theme_export)],
                    input=b"", capture_output=True, timeout=15)
                assert imported.returncode == 0 and imported.stdout == b"8be9fd", (imported.stdout, imported.stderr)
                web_stage("browser theme edit persisted")
                page.locator("#tab_prompt").click()
                page.locator(".prompt_choices_list > div").filter(has=page.locator(".prompt_demo_choice_label", has_text=re.compile("^Simple$"))).locator(".prompt_demo").first.click()
                page.get_by_role("button", name="Set Prompt", exact=True).click()
                page.get_by_role("button", name="Prompt Set!", exact=True).wait_for()
                assert prompt.is_file()
                selected = subprocess.run(
                    [str(FISH), "-c", "set -gx USER 'N9 UI 雪'; set -g hostname n9.example; fish_prompt"],
                    input=b"", capture_output=True, timeout=15)
                assert selected.returncode == 0 and "N9 UI 雪@n9.example".encode() in selected.stdout, (selected.stdout, selected.stderr)
                web_stage("browser Simple prompt persisted")
                # Exercise the actual browser's prompt state and save button
                # with Unicode source, not just an out-of-browser HTTP edit.
                page.locator("[x-data='prompt']").evaluate(
                    "(element, source) => { const state = window.Alpine.$data(element); "
                    "state.selectPrompt({...state.selectedPrompt, function: source}); }",
                    edit["fish_prompt"])
                page.get_by_role("button", name="Set Prompt", exact=True).click()
                page.get_by_role("button", name="Prompt Set!", exact=True).wait_for()
                saved_theme = frozen.read_bytes()
                selected = subprocess.run([str(FISH), "-c", "fish_prompt"], input=b"", capture_output=True, timeout=15)
                assert selected.returncode == 0 and selected.stdout == "N9 web 雪> ".encode(), (selected.stdout, selected.stderr)
                web_stage("browser Unicode prompt persisted")
                # The functions tab goes through the matching builtin formatter.
                page.locator("#tab_functions").click()
                page.get_by_text("fish_prompt", exact=True).first.click()
                page.locator(".detail_function").filter(has_text="N9 web 雪>").wait_for()
                page.reload()
                page.locator(".color_scheme_choice_container").first.wait_for()
                assert frozen.read_bytes() == saved_theme
                png = page.screenshot(path=str(screenshot))
                assert png[:8] == b"\x89PNG\r\n\x1a\n" and len(png) > 1000
                assert int.from_bytes(png[16:20], "big") == 800
                assert int.from_bytes(png[20:24], "big") == 600
                web_stage("function inspection and reload complete")
            except BaseException:
                # Persist the actual launch/UI error before driver shutdown;
                # a timed-out Edge launch can leave that shutdown waiting on
                # its still-running child and hide the original exception.
                (ROOT / "web-browser-error.txt").write_text(traceback.format_exc(), encoding="utf-8")
                raise
            finally:
                try:
                    # Retain the real browser's native handle rather than wait
                    # for an unbounded CDP Browser.close acknowledgement.
                    web_stage("native browser cleanup start")
                    if launched is not None:
                        if launched.poll() is None:
                            launched.terminate()
                            assert launched.wait(timeout=5) == 1
                        else:
                            assert launched.returncode == 0
                    if browser_process is not None:
                        try:
                            if kernel.WaitForSingleObject(browser_process, 0) == 258:
                                assert kernel.TerminateProcess(browser_process, 1)
                            assert kernel.WaitForSingleObject(browser_process, 5000) == 0
                        finally:
                            kernel.CloseHandle(browser_process)
                    if browser is not None:
                        browser.close()
                    else:
                        # Failed launch never returned a browser handle.
                        # All of these children belong to this probe's job;
                        # close them before Playwright waits for its transport.
                        finish_owned_children(ownership._JOB)
                    web_stage("native browser cleanup complete")
                finally:
                    if saved_browser is None:
                        os.environ.pop("BROWSER", None)
                    else:
                        os.environ["BROWSER"] = saved_browser
        assert request(server, prefix + "/set_prompt/", edit)[0] == 200
        result = subprocess.run([str(FISH), "-c", "fish_prompt"], input=b"", capture_output=True, timeout=15)
        assert result.returncode == 0 and result.stdout == "N9 web 雪> ".encode(), (result.stdout, result.stderr)
        before = (prompt.read_bytes(), frozen.read_bytes())
        assert request(server, "/invalid/set_prompt/", {"fish_prompt": "function fish_prompt; echo denied; end"})[0] == 403
        assert request(server, "/invalid/set_color/", {"theme": "denied 雪", "colors": []})[0] == 403
        assert before == (prompt.read_bytes(), frozen.read_bytes())
        web_stage("post-edit rejection and persistence complete")
    finally:
        server.shutdown()
        worker.join(timeout=5)
        server.server_close()
        os.chdir(old_cwd)
    assert not worker.is_alive()
    assert not any(thread.name.startswith("Thread-") and thread.is_alive() for thread in threading.enumerate())
    assert not listener_pids("127.0.0.1", server.server_address[1]), "web listening socket leaked"


def nonloopback_ipv4_address():
    """Discover an assigned native interface, without DNS or network changes."""
    import ctypes

    class AddressRow(ctypes.Structure):
        _fields_ = [
            ("address", ctypes.c_ubyte * 4), ("index", ctypes.c_uint32),
            ("mask", ctypes.c_uint32), ("broadcast", ctypes.c_uint32),
            ("reassembly_size", ctypes.c_uint32), ("unused", ctypes.c_uint16),
            ("type", ctypes.c_uint16),
        ]

    api = ctypes.WinDLL("iphlpapi", use_last_error=True).GetIpAddrTable
    api.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32), ctypes.c_int]
    api.restype = ctypes.c_uint32
    size = ctypes.c_uint32()
    result = api(None, ctypes.byref(size), False)
    assert result == 122, ("GetIpAddrTable sizing", result)
    table = ctypes.create_string_buffer(size.value)
    result = api(table, ctypes.byref(size), False)
    assert result == 0, ("GetIpAddrTable", result)
    count = ctypes.c_uint32.from_buffer(table).value
    assert 4 + count * ctypes.sizeof(AddressRow) <= size.value
    assigned = []
    for index in range(count):
        row = AddressRow.from_buffer(table, 4 + index * ctypes.sizeof(AddressRow))
        address = socket.inet_ntop(socket.AF_INET, bytes(row.address))
        assigned.append((address, row.type))
    usable = sorted(address for address, kind in assigned
                    if address != "0.0.0.0" and not address.startswith("127.")
                    and not kind & (0x8 | 0x40))  # DISCONNECTED / DELETED
    assert usable, ("required reachable nonloopback IPv4 fixture unavailable", assigned)
    return usable[0]


def listener_pids(host, port):
    """Read the native TCP listener table, independent of connect error codes."""
    import ctypes

    class Row4(ctypes.Structure):
        _fields_ = [
            ("state", ctypes.c_uint32), ("local_address", ctypes.c_ubyte * 4),
            ("local_port", ctypes.c_uint32), ("remote_address", ctypes.c_ubyte * 4),
            ("remote_port", ctypes.c_uint32), ("pid", ctypes.c_uint32),
        ]

    class Row6(ctypes.Structure):
        _fields_ = [
            ("local_address", ctypes.c_ubyte * 16), ("local_scope", ctypes.c_uint32),
            ("local_port", ctypes.c_uint32), ("remote_address", ctypes.c_ubyte * 16),
            ("remote_scope", ctypes.c_uint32), ("remote_port", ctypes.c_uint32),
            ("state", ctypes.c_uint32), ("pid", ctypes.c_uint32),
        ]

    family, row_type = (socket.AF_INET6, Row6) if ":" in host else (socket.AF_INET, Row4)
    api = ctypes.WinDLL("iphlpapi", use_last_error=True).GetExtendedTcpTable
    api.argtypes = [
        ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32), ctypes.c_int,
        ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32,
    ]
    api.restype = ctypes.c_uint32
    size = ctypes.c_uint32()
    # TCP_TABLE_OWNER_PID_LISTENER = 3; the first call sizes the native table.
    result = api(None, ctypes.byref(size), False, family, 3, 0)
    assert result == 122, ("GetExtendedTcpTable sizing", result)
    table = ctypes.create_string_buffer(size.value)
    result = api(table, ctypes.byref(size), False, family, 3, 0)
    assert result == 0, ("GetExtendedTcpTable", result)
    count = ctypes.c_uint32.from_buffer(table).value
    assert 4 + count * ctypes.sizeof(row_type) <= size.value
    owners = set()
    for index in range(count):
        row = row_type.from_buffer(table, 4 + index * ctypes.sizeof(row_type))
        assert row.state == 2, ("non-LISTEN TCP row", row.state)
        if (socket.inet_ntop(family, bytes(row.local_address)) == host
                and socket.ntohs(row.local_port & 0xFFFF) == port):
            owners.add(row.pid)
    return owners


def read_started(process, require_live=True):
    lines = queue.Queue()
    def consume():
        for line in process.stdout:
            lines.put(line.decode("utf-8", "replace"))
    worker = threading.Thread(target=consume, name="n9-read-web-output")
    worker.start()
    redirect = None
    address = None
    for _ in range(8):
        line = lines.get(timeout=10)
        match = re.search(r"file://[^\x1b\r\n]+", line)
        if match:
            from urllib.request import url2pathname
            redirect = Path(url2pathname(urlsplit(match.group()).path))
        url = re.search(r"http://[^\x1b\r\n]+", line)
        if url:
            address = urlsplit(url.group())
        if "Hit ENTER" in line:
            assert redirect and address
            if require_live:
                assert redirect.is_file()
            return redirect, address, worker
    raise AssertionError("web service did not announce readiness")


def web_lifetime():
    import _winapi
    case = ROOT / "browser junction ownership"
    home = case / "profile home 雪"
    cache = home / "AppData/Local/Microsoft/Windows/INetCache"
    target = cache / "IE"
    target.mkdir(parents=True)
    receipt = target / "kept.txt"
    receipt.write_bytes("owned cache 雪".encode("utf-8"))
    junction = cache / "Content.IE5"
    _winapi.CreateJunction(str(target), str(junction))
    remove_owned_browser_junction(case, home)
    assert not junction.exists() and receipt.read_bytes() == "owned cache 雪".encode("utf-8")
    outside = ROOT / "outside junction target 雪"
    outside.mkdir()
    protected = outside / "kept.txt"
    protected.write_bytes(b"outside data must survive")
    _winapi.CreateJunction(str(outside), str(junction))
    try:
        try:
            remove_owned_browser_junction(case, home)
        except AssertionError:
            pass
        else:
            raise AssertionError("unexpected junction target was accepted")
        assert junction.samefile(outside)
        assert protected.read_bytes() == b"outside data must survive"
    finally:
        junction.rmdir()
    shutil.rmtree(case)
    shutil.rmtree(outside)
    # Exercise the public extraction/interpreter/dispatch consumer, not import.
    import ctypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    owned_console = kernel.GetConsoleCP() == 0
    if owned_console:
        assert kernel.AllocConsole(), ctypes.WinError(ctypes.get_last_error())
    try:
        for shutdown in ("enter", "eof", "cancel", "browser-failure"):
            env = dict(os.environ, PATH=str(PYTHON.parent), BROWSER="true", PYTHONUNBUFFERED="1", fish_bin_path=str(FISH))
            if shutdown == "browser-failure":
                env["BROWSER"] = str(ROOT / "missing native browser.exe")
            argv = [str(PYTHON), "-B", str(WEB)] if shutdown == "cancel" else [str(FISH), "-N", "-c", "fish_config browse"]
            process = subprocess.Popen(argv, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, creationflags=subprocess.CREATE_NEW_PROCESS_GROUP)
            redirect = output_worker = None
            try:
                redirect, address, output_worker = read_started(process, shutdown != "browser-failure")
                if shutdown != "browser-failure":
                    owners = listener_pids(address.hostname, address.port)
                    assert len(owners) == 1 and os.getpid() not in owners, ("missing native web listener", owners)
                    connection = http.client.HTTPConnection(address.hostname, address.port, timeout=10)
                    try:
                        connection.request("GET", address.path)
                        response = connection.getresponse()
                        body = response.read()
                        assert response.status == 200, (shutdown, response.status, address.geturl(), body)
                    finally:
                        connection.close()
                if shutdown == "enter":
                    process.stdin.write(b"\n")
                    process.stdin.flush()
                elif shutdown == "cancel":
                    process.send_signal(signal.CTRL_BREAK_EVENT)
                if shutdown != "cancel":
                    process.stdin.close()
                assert process.wait(timeout=15) == (1 if shutdown == "browser-failure" else 0), process.stderr.read()
                if not process.stdin.closed:
                    process.stdin.close()
                output_worker.join(timeout=5)
                assert not output_worker.is_alive()
                assert not redirect.exists() and not redirect.parent.exists()
                assert not listener_pids(address.hostname, address.port), (
                    shutdown, "web listening socket leaked", address.hostname, address.port)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
                if output_worker is not None:
                    output_worker.join(timeout=5)
                if not process.stdin.closed:
                    process.stdin.close()
                process.stdout.close()
                process.stderr.close()
    finally:
        if owned_console:
            assert kernel.FreeConsole()


def finish_owned_children(job):
    """Retain and stop only remaining members of this probe's own native job."""
    import ctypes
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.QueryInformationJobObject.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p,
                                                ctypes.c_uint32, ctypes.c_void_p]
    kernel.OpenProcess.argtypes = [ctypes.c_uint32, ctypes.c_int, ctypes.c_uint32]
    kernel.OpenProcess.restype = ctypes.c_void_p
    kernel.IsProcessInJob.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_int)]
    kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel.WaitForSingleObject.restype = ctypes.c_uint32
    kernel.TerminateProcess.argtypes = [ctypes.c_void_p, ctypes.c_uint32]
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    size = 512
    while True:
        members = ctypes.create_string_buffer(size)
        if kernel.QueryInformationJobObject(job, 3, members, size, None):
            break
        error = ctypes.get_last_error()
        if error != 234:  # ERROR_MORE_DATA: size the native PID-list buffer.
            raise ctypes.WinError(error)
        assigned = ctypes.c_uint32.from_buffer(members).value
        size = max(size * 2, 8 + assigned * ctypes.sizeof(ctypes.c_size_t))
    count = ctypes.c_uint32.from_buffer(members, 4).value
    assert 8 + count * ctypes.sizeof(ctypes.c_size_t) <= size
    retained = []
    try:
        for index in range(count):
            pid = ctypes.c_size_t.from_buffer(members, 8 + index * ctypes.sizeof(ctypes.c_size_t)).value
            if pid == os.getpid():
                continue
            process = kernel.OpenProcess(0x100000 | 0x1000 | 1, False, pid)
            if not process:
                error = ctypes.get_last_error()
                if error == 87:  # The snapshotted process already exited.
                    continue
                raise ctypes.WinError(error)
            retained.append((pid, process))
            member = ctypes.c_int()
            if not kernel.IsProcessInJob(process, job, ctypes.byref(member)):
                raise ctypes.WinError(ctypes.get_last_error())
            if not member.value:
                # A recycled PID is not authority to touch a foreign process.
                retained.pop()
                kernel.CloseHandle(process)
                continue
            if kernel.WaitForSingleObject(process, 0) == 258:
                if not kernel.TerminateProcess(process, 1):
                    assert kernel.WaitForSingleObject(process, 0) == 0, ctypes.WinError(ctypes.get_last_error())
        for pid, process in retained:
            assert kernel.WaitForSingleObject(process, 5000) == 0, ("owned descendant did not exit", pid)
    finally:
        for _, process in retained:
            kernel.CloseHandle(process)


MODES = {"interpreter": interpreter, "manpages": manpages, "background": background, "web-edits": web_edits, "web-lifetime": web_lifetime}
web_mode = sys.argv[1] in ("web-edits", "web-lifetime")
if web_mode:
    spec = importlib.util.spec_from_file_location("fish_n9_owned_evidence", SOURCE / "tests/windows_n11/evidence.py")
    ownership = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(ownership)
    ownership.owned_job()
    import faulthandler
    stacks = (ROOT / "web-live-stacks.txt").open("w", encoding="utf-8")
    faulthandler.dump_traceback_later(10, repeat=True, file=stacks)
try:
    MODES[sys.argv[1]]()
finally:
    if web_mode:
        try:
            web_stage("owned descendant cleanup start")
            finish_owned_children(ownership._JOB)
            remove_owned_browser_junction(ROOT, Path(os.environ["USERPROFILE"]))
            profile = ROOT / "owned native browser profile 雪"
            if profile.exists():
                shutil.rmtree(profile)
            (ROOT / "native browser redirect.html").unlink(missing_ok=True)
            web_stage("owned descendant cleanup complete")
        finally:
            faulthandler.cancel_dump_traceback_later()
            stacks.close()
print("N9 tool consumer passed: " + sys.argv[1])
