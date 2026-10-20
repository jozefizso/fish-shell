import binascii
import errno
import shlex
import glob
import http.server as SimpleHTTPServer
import json
import multiprocessing.pool
import operator
import os
import platform
import re
import select
import signal
import socket
import socketserver as SocketServer
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path
from html import escape as escape_html
from itertools import chain
from typing import Optional
from urllib.parse import parse_qs

COMMON_WSL_CMD_PATHS = (
    "/mnt/c/Windows/System32",
    "/windir/c/Windows/System32",
    "/c/Windows/System32",
)
FISH_BIN_PATH = False  # will be set later


ENTER_BOLD_MODE = "\x1b[1m"
EXIT_ATTRIBUTE_MODE = "\x1b[m"
ENTER_UNDERLINE_MODE = "\x1b[4m"

# Disable CLI web browsers
term = os.environ.pop("TERM", None)
# This import must be done with an empty $TERM, otherwise a command-line browser may be started
# which will block the whole process - see https://docs.python.org/3/library/webbrowser.html
import webbrowser

if term is not None:
    os.environ["TERM"] = term

TERMINAL_COLOR_THEME = os.environ.get("__fish_terminal_color_theme", "unknown")


def find_executable(exe, paths=()):
    final_path = os.environ.get("PATH", "").split(os.pathsep)
    if paths:
        final_path.extend(paths)
    for p in final_path:
        if not p:
            continue
        proposed_path = os.path.join(p, exe)
        if os.access(proposed_path, os.X_OK):
            return proposed_path


def is_wsl():
    """Return whether we are running under the Windows Subsystem for Linux"""
    if "linux" in platform.system().lower() and os.access("/proc/version", os.R_OK):
        with open("/proc/version", "r") as f:
            # Find 'Microsoft' for wsl1 and 'microsoft' for wsl2
            if "microsoft" in f.read().lower():
                return True
    return False


def is_windows():
    """Return whether we are running under the Windows"""
    return sys.platform.startswith("win")


def is_sailfish_os():
    """Return whether we are running on Sailfish OS"""
    if "linux" in platform.system().lower() and os.access(
        "/etc/sailfish-release", os.R_OK
    ):
        with open("/etc/sailfish-release", "r") as f:
            # Find 'ID=sailfishos'
            if "sailfishos" in f.read():
                return True
    return False


def is_termux():
    """Return whether we are running under the Termux application for Android"""
    return "com.termux" in os.environ.get("PATH", "") and find_executable("termux-open-url")


def is_chromeos_garcon():
    """Return whether we are running in Chrome OS and the browser can't see local files"""
    # In Crostini Chrome OS Linux, the default browser opens URLs in Chrome
    # running outside the linux VM. This browser does not have access to the
    # Linux filesystem. This uses Garcon, see for example
    # https://chromium.googlesource.com/chromiumos/platform2/+/master/vm_tools/garcon/#opening-urls
    # https://source.chromium.org/search?q=garcon-url-handler
    try:
        return "garcon-url-handler" in webbrowser.get().name
    except:
        return False


def run_fish_cmd(text, strict=False, env=None):
    print("$ " + text)
    p = subprocess.Popen(
        [FISH_BIN_PATH],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    try:
        out, err = p.communicate(text.encode("utf-8"), timeout=30)
    except BaseException:
        p.kill()
        p.communicate()
        raise
    out = out.decode("utf-8", "replace")
    err = err.decode("utf-8", "replace")
    if strict:
        if p.returncode or err:
            raise RuntimeError(err or "fish exited with status %d" % p.returncode)
        return out
    return out, err


def list_embedded_files(path, suffix):
    return [
        path
        for path in run_fish_cmd(
            "status list-files " + escape_fish_cmd(path), strict=True
        ).splitlines()
        if path.endswith(suffix)
    ]


def get_embedded_file(path):
    return run_fish_cmd("status get-file " + escape_fish_cmd(path), strict=True)


def escape_fish_cmd(text):
    # Replace one backslash with two, and single quotes with backslash-quote
    escaped = text.replace("\\", "\\\\").replace("'", "\\'")
    return "'" + escaped + "'"


def strip_one_layer(text, char):
    # Strip the text from one layer of a given character
    if text[-1] == char:
        text = text[:-1]
    if text[0] == char:
        text = text[1:]
    return text


named_colors = {
    "black": "000000",
    "red": "800000",
    "green": "008000",
    "brown": "725000",
    "yellow": "808000",
    "blue": "000080",
    "magenta": "800080",
    "purple": "800080",
    "cyan": "008080",
    "grey": "e5e5e5",
    "brgrey": "555555",
    "white": "c0c0c0",
    "brblack": "808080",
    "brred": "ff0000",
    "brgreen": "00ff00",
    "brbrown": "ffff00",
    "bryellow": "ffff00",
    "brblue": "0000ff",
    "brmagenta": "ff00ff",
    "brpurple": "ff00ff",
    "brcyan": "00ffff",
    "brwhite": "ffffff",
}

bindings_blacklist = set(["self-insert", "'begin;end'"])


def parse_one_color(comp):
    """A basic function to parse a single color value like 'FFA000'"""
    if comp in named_colors:
        # Named color
        return named_colors[comp]
    elif (
        re.match(r"[0-9a-fA-F]{3}", comp) is not None
        or re.match(r"[0-9a-fA-F]{6}", comp) is not None
    ):
        # Hex color
        return comp
    else:
        # Unknown
        return ""


def better_color(c1, c2):
    """Indicate which color is "better", i.e. prefer term256 colors"""
    if not c2:
        return c1
    if not c1:
        return c2
    if c1 == "normal":
        return c2
    if c2 == "normal":
        return c1
    if c2 in named_colors:
        return c1
    if c1 in named_colors:
        return c2
    return c1


def parse_colors(colors_str):
    for words in map(shlex.split, colors_str.splitlines()):
        assert re.match(
            r"^((?:fish_color_|fish_pager_color_)\w+)$",
            words[0],
        )
    return {
        "colors": sorted(
            [
                {"name": words[0]} | parse_color(words[1:])
                for words in map(shlex.split, colors_str.splitlines())
            ],
            key=operator.itemgetter("name"),
        ),
    }


def parse_color(comps):
    """A basic function to parse a color string, for example, ['red', '--bold']."""
    color = ""
    background_color = ""
    underline_color = ""
    bold = False
    underline = None
    italics = False
    strikethrough = False
    dim = False
    reverse = False
    i = 0
    while i < len(comps):
        comp = comps[i]
        if comp == "--bold" or comp == "-o":
            bold = True
        elif comp == "--underline" or comp == "-u":
            underline = "single"
        elif comp.startswith("--underline="):
            # TODO(python>3.8): use removeprefix
            underline = comp[len("--underline=") :]
        elif comp.startswith(
            "-u"
        ):  # Multiple short options like "-rucurly" are not yet supported.
            # TODO(python>3.8): use removeprefix
            underline = comp[len("-u") :]
        elif comp == "--italics" or comp == "-i":
            italics = True
        elif comp == "--dim" or comp == "-d":
            dim = True
        elif comp == "--reverse" or comp == "-r":
            reverse = True
        elif comp == "--strikethrough" or comp == "-s":
            strikethrough = True
        elif comp.startswith("--theme="):
            pass  # Not yet supported here.
        else:

            def parse_opt(
                current_best: str, i: int, long_opt: str, short_opt: Optional[str]
            ) -> str:
                if comp.startswith(long_opt):
                    c = comp[len(long_opt) :]
                    if c[0] == "=":
                        # There was a = between the long option and the value.
                        # i.e. support also --background=red, not just --background red
                        c = c[1:]
                    parsed_c = parse_one_color(c)
                    # We prefer the unparsed version - if it says "brgreen", we use brgreen,
                    # instead of 00ff00
                    if better_color(current_best, parsed_c) == parsed_c:
                        return True, c, i
                elif short_opt is not None and comp.startswith(short_opt):
                    if comp == short_opt:
                        if i + 1 == len(comps):
                            c = ""
                        else:
                            c = comps[i + 1]
                            i += 1
                    else:
                        c = comp[len(short_opt) :]
                    parsed_c = parse_one_color(c)
                    if better_color(current_best, parsed_c) == parsed_c:
                        return True, c, i
                return False, current_best, i

            is_bg, background_color, i = parse_opt(
                background_color, i, "--background", "-b"
            )
            is_ul, underline_color, i = parse_opt(
                underline_color, i, "--underline-color", None
            )
            if not (is_bg or is_ul):
                # Regular color
                parsed_c = parse_one_color(comp)
                if better_color(color, parsed_c) == parsed_c:
                    color = comp
        i += 1

    return {
        "color": color,
        "background": background_color,
        "underline-color": underline_color,
        "bold": bold,
        "underline": underline,
        "italics": italics,
        "dim": dim,
        "reverse": reverse,
        "strikethrough": strikethrough,
    }


def unparse_color(col):
    """A basic function to return the fish version of a color dict"""
    if isinstance(col, str):
        return col
    ret = ""
    if col["color"]:
        ret += col["color"]
    if col["bold"]:
        ret += " --bold"
    if col["underline"] is not None:
        ret += " --underline=" + str(col["underline"])
    if col["italics"]:
        ret += " --italics"
    if col["dim"]:
        ret += " --dim"
    if col["reverse"]:
        ret += " --reverse"
    if col["strikethrough"]:
        ret += " --strikethrough"
    if col["background"]:
        ret += " --background=" + col["background"]
    if col["underline-color"]:
        ret += " --underline-color=" + col["underline-color"]
    return ret


def parse_bool(val):
    val = val.lower()
    if val.startswith("f") or val.startswith("0"):
        return False
    if val.startswith("t") or val.startswith("1"):
        return True
    return bool(val)


def html_color_for_ansi_color_index(val):
    arr = [
        "black",
        "#FF0000",
        "#00FF00",
        "#AA5500",
        "#0000FF",
        "#AA00AA",
        "#00AAAA",
        "#AAAAAA",
        "#555555",
        "#FF5555",
        "#55FF55",
        "#FFFF55",
        "#5555FF",
        "#FF55FF",
        "#55FFFF",
        "white",
        "#000000",
        "#00005f",
        "#000087",
        "#0000af",
        "#0000d7",
        "#0000ff",
        "#005f00",
        "#005f5f",
        "#005f87",
        "#005faf",
        "#005fd7",
        "#005fff",
        "#008700",
        "#00875f",
        "#008787",
        "#0087af",
        "#0087d7",
        "#0087ff",
        "#00af00",
        "#00af5f",
        "#00af87",
        "#00afaf",
        "#00afd7",
        "#00afff",
        "#00d700",
        "#00d75f",
        "#00d787",
        "#00d7af",
        "#00d7d7",
        "#00d7ff",
        "#00ff00",
        "#00ff5f",
        "#00ff87",
        "#00ffaf",
        "#00ffd7",
        "#00ffff",
        "#5f0000",
        "#5f005f",
        "#5f0087",
        "#5f00af",
        "#5f00d7",
        "#5f00ff",
        "#5f5f00",
        "#5f5f5f",
        "#5f5f87",
        "#5f5faf",
        "#5f5fd7",
        "#5f5fff",
        "#5f8700",
        "#5f875f",
        "#5f8787",
        "#5f87af",
        "#5f87d7",
        "#5f87ff",
        "#5faf00",
        "#5faf5f",
        "#5faf87",
        "#5fafaf",
        "#5fafd7",
        "#5fafff",
        "#5fd700",
        "#5fd75f",
        "#5fd787",
        "#5fd7af",
        "#5fd7d7",
        "#5fd7ff",
        "#5fff00",
        "#5fff5f",
        "#5fff87",
        "#5fffaf",
        "#5fffd7",
        "#5fffff",
        "#870000",
        "#87005f",
        "#870087",
        "#8700af",
        "#8700d7",
        "#8700ff",
        "#875f00",
        "#875f5f",
        "#875f87",
        "#875faf",
        "#875fd7",
        "#875fff",
        "#878700",
        "#87875f",
        "#878787",
        "#8787af",
        "#8787d7",
        "#8787ff",
        "#87af00",
        "#87af5f",
        "#87af87",
        "#87afaf",
        "#87afd7",
        "#87afff",
        "#87d700",
        "#87d75f",
        "#87d787",
        "#87d7af",
        "#87d7d7",
        "#87d7ff",
        "#87ff00",
        "#87ff5f",
        "#87ff87",
        "#87ffaf",
        "#87ffd7",
        "#87ffff",
        "#af0000",
        "#af005f",
        "#af0087",
        "#af00af",
        "#af00d7",
        "#af00ff",
        "#af5f00",
        "#af5f5f",
        "#af5f87",
        "#af5faf",
        "#af5fd7",
        "#af5fff",
        "#af8700",
        "#af875f",
        "#af8787",
        "#af87af",
        "#af87d7",
        "#af87ff",
        "#afaf00",
        "#afaf5f",
        "#afaf87",
        "#afafaf",
        "#afafd7",
        "#afafff",
        "#afd700",
        "#afd75f",
        "#afd787",
        "#afd7af",
        "#afd7d7",
        "#afd7ff",
        "#afff00",
        "#afff5f",
        "#afff87",
        "#afffaf",
        "#afffd7",
        "#afffff",
        "#d70000",
        "#d7005f",
        "#d70087",
        "#d700af",
        "#d700d7",
        "#d700ff",
        "#d75f00",
        "#d75f5f",
        "#d75f87",
        "#d75faf",
        "#d75fd7",
        "#d75fff",
        "#d78700",
        "#d7875f",
        "#d78787",
        "#d787af",
        "#d787d7",
        "#d787ff",
        "#d7af00",
        "#d7af5f",
        "#d7af87",
        "#d7afaf",
        "#d7afd7",
        "#d7afff",
        "#d7d700",
        "#d7d75f",
        "#d7d787",
        "#d7d7af",
        "#d7d7d7",
        "#d7d7ff",
        "#d7ff00",
        "#d7ff5f",
        "#d7ff87",
        "#d7ffaf",
        "#d7ffd7",
        "#d7ffff",
        "#ff0000",
        "#ff005f",
        "#ff0087",
        "#ff00af",
        "#ff00d7",
        "#ff00ff",
        "#ff5f00",
        "#ff5f5f",
        "#ff5f87",
        "#ff5faf",
        "#ff5fd7",
        "#ff5fff",
        "#ff8700",
        "#ff875f",
        "#ff8787",
        "#ff87af",
        "#ff87d7",
        "#ff87ff",
        "#ffaf00",
        "#ffaf5f",
        "#ffaf87",
        "#ffafaf",
        "#ffafd7",
        "#ffafff",
        "#ffd700",
        "#ffd75f",
        "#ffd787",
        "#ffd7af",
        "#ffd7d7",
        "#ffd7ff",
        "#ffff00",
        "#ffff5f",
        "#ffff87",
        "#ffffaf",
        "#ffffd7",
        "#ffffff",
        "#080808",
        "#121212",
        "#1c1c1c",
        "#262626",
        "#303030",
        "#3a3a3a",
        "#444444",
        "#4e4e4e",
        "#585858",
        "#626262",
        "#6c6c6c",
        "#767676",
        "#808080",
        "#8a8a8a",
        "#949494",
        "#9e9e9e",
        "#a8a8a8",
        "#b2b2b2",
        "#bcbcbc",
        "#c6c6c6",
        "#d0d0d0",
        "#dadada",
        "#e4e4e4",
        "#eeeeee",
    ]
    if val < 0 or val >= len(arr):
        return ""
    else:
        return arr[val]


# Given a known ANSI escape sequence, convert it to HTML and append to the list
# Returns whether we have an open <span>


def append_html_for_ansi_escape(full_val, result, span_open):
    # Strip off the initial \x1b[ and terminating m
    val = full_val[2:-1]

    # Helper function to close a span if it's open
    def close_span():
        if span_open:
            result.append("</span>")

    # term24bit foreground color
    match = re.match(r"38;2;(\d+);(\d+);(\d+)", val)
    if match is not None:
        close_span()
        # Just use the rgb values directly
        html_color = "#%02x%02x%02x" % (
            int(match.group(1)),
            int(match.group(2)),
            int(match.group(3)),
        )
        result.append('<span style="color: ' + html_color + '">')
        return True  # span now open

    # term256 foreground color
    match = re.match(r"38;5;(\d+)", val)
    if match is not None:
        close_span()
        html_color = html_color_for_ansi_color_index(int(match.group(1)))
        result.append('<span style="color: ' + html_color + '">')
        return True  # span now open

    # term16 foreground color
    if val in (str(x) for x in chain(range(90, 97), range(30, 38))):
        close_span()
        html_color = html_color_for_ansi_color_index(
            int(val) - (30 if int(val) < 90 else 82)
        )
        result.append('<span style="color: ' + html_color + '">')
        return True  # span now open

    # Try special escapes
    if full_val == EXIT_ATTRIBUTE_MODE:
        close_span()
        return False

    # TODO We don't handle bold, underline, italics, dim, strikethrough, or reverse yet

    # Do nothing on failure
    return span_open


def strip_ansi(val):
    # Make a half-assed effort to strip ANSI control sequences
    # We assume that all such sequences start with 0x1b and end with m or ctrl-o,
    # which catches most cases
    return re.sub("\x1b[^m]*m\x0f?", "", val)


def ansi_prompt_line_width(val):
    # Given an ANSI prompt, return the length of its longest line, as in the
    # number of characters it takes up. Start by stripping off ANSI.
    stripped_val = strip_ansi(val)

    # Now count the longest line
    return max([len(x) for x in stripped_val.split("\n")])


def ansi_to_html(val):
    # Split us up by ANSI escape sequences. We want to catch not only the
    # standard color codes, but also things like sgr0. Hence this lame check.
    # Note that Python 2.6 doesn't have a flag param to re.split, so we have to
    # compile it first.
    reg = re.compile(
        """
        (                        # Capture
         \x1b                    # Escape
         [^m]*                   # Zero or more non-'m's
         m                       # Literal m terminates the sequence
         \x0f?                   # HACK: A ctrl-o - this is how tmux' sgr0 ends
        )                        # End capture
        """,
        re.VERBOSE,
    )
    separated = reg.split(val)

    # We have to HTML escape the text and convert ANSI escapes into HTML
    # Collect it all into this array
    result = []

    span_open = False

    # Text is at even indexes, escape sequences at odd indexes
    for i in range(len(separated)):
        component = separated[i]
        if i % 2 == 0:
            # It's text, possibly empty
            # Clean up other ANSI junk
            result.append(escape_html(strip_ansi(component)))
        else:
            # It's an escape sequence. Close the previous escape.
            span_open = append_html_for_ansi_escape(component, result, span_open)

    # Close final escape
    if span_open:
        result.append("</span>")

    # Remove empty elements
    result = [x for x in result if x]

    # Clean up empty spans, the nasty way
    idx = len(result) - 1
    while idx >= 1:
        if result[idx] == "</span>" and result[idx - 1].startswith("<span"):
            # Empty span, delete these two
            result[idx - 1 : idx + 1] = []
            idx = idx - 1
        idx = idx - 1

    return "".join(result)


class FishVar:
    """A class that represents a variable"""

    def __init__(self, name, value):
        self.name = name
        self.value = value
        self.universal = False
        self.exported = False

    def get_json_obj(self):
        # Return an array(3): name, value, flags
        flags = []
        if self.universal:
            flags.append("universal")
        if self.exported:
            flags.append("exported")
        return {"name": self.name, "value": self.value, "Flags": ", ".join(flags)}


class FishBinding:
    """A class that represents keyboard binding"""

    def __init__(self, command, raw_binding, readable_binding):
        self.command = command
        self.bindings = []
        self.add_binding(raw_binding, readable_binding)

    def add_binding(self, raw_binding, readable_binding):
        for i in self.bindings:
            if i["readable_binding"] == readable_binding:
                i["raw_bindings"].append(raw_binding)
                break
        else:
            self.bindings.append(
                {"readable_binding": readable_binding, "raw_bindings": [raw_binding]}
            )

    def get_json_obj(self):
        return {
            "command": self.command,
            "bindings": self.bindings,
        }


class FishConfigTCPServer(SocketServer.ThreadingMixIn, SocketServer.TCPServer):
    """TCPServer that only accepts connections from localhost (IPv4/IPv6)."""

    WHITELIST = set(["::1", "::ffff:127.0.0.1", "127.0.0.1"])

    address_family = socket.AF_INET6 if socket.has_ipv6 else socket.AF_INET

    daemon_threads = False
    block_on_close = True

    def get_request(self):
        request, address = super().get_request()
        # A browser may preconnect without ever sending HTTP headers.
        request.settimeout(2)
        return request, address
    def verify_request(self, request, client_address):
        return client_address[0] in FishConfigTCPServer.WHITELIST


class FishConfigHTTPRequestHandler(SimpleHTTPServer.SimpleHTTPRequestHandler):
    # These are the assets shipped with webconfig. Do not initialize the host
    # MIME database (registry/system-file probes) merely to serve private assets.
    extensions_map = {
        ".html": "text/html",
        ".css": "text/css",
        ".js": "text/javascript",
        ".png": "image/png",
    }

    def guess_type(self, path):
        return self.extensions_map.get(os.path.splitext(path)[1].lower(), "application/octet-stream")

    def translate_path(self, path):
        translated = super().translate_path(path)
        # The stdlib appends a literal "/" for directory URLs. Windows extended
        # paths require native separators even for that final slash.
        return translated.replace("/", os.sep) if is_windows() else translated

    def write_to_wfile(self, txt):
        self.wfile.write(txt.encode("utf-8"))

    def do_get_colors(self):
        out, err = run_fish_cmd(
            ""
            + "if set -q __fish_initialized && test $__fish_initialized -lt 4300\n"
            + "    echo >&2 missing migration?\n"
            + "    exit 1\n"
            + "end\n"
            + "__fish_color_theme={} __fish_apply_theme 2>/dev/null\n".format(
                TERMINAL_COLOR_THEME
            )
            + "or __fish_color_theme=unknown __fish_apply_theme\n"
            + "__fish_theme_export_for_webconfig",
            env=os.environ | {"__fish_force_load_default_theme": "1"},
        )
        assert err == "", err

        words = iter(shlex.split(out))
        current = next(words)
        themes = [
            {
                "theme": "Current",
                "by_color_theme": {
                    # Hack-ish: we don't know about the light/dark variants.
                    "unknown": parse_colors(current)
                    | {
                        "preferred_background": "white"
                        if TERMINAL_COLOR_THEME == "light"
                        else "black",
                    }
                },
                "terminal_color_theme": TERMINAL_COLOR_THEME,
            }
        ]
        while True:
            try:
                theme_name = next(words)
            except StopIteration:
                break
            theme = {
                "theme": theme_name,
            }
            pretty_name = next(words)
            if pretty_name:
                theme["name"] = pretty_name
            url = next(words)
            if url:
                theme["url"] = url
            by_color_theme = {}
            for _i in range(int(next(words))):
                color_theme = next(words)
                preferred_background = next(words)
                data = next(words)
                by_color_theme[color_theme] = parse_colors(data) | {
                    "preferred_background": preferred_background
                }
            theme["by_color_theme"] = by_color_theme
            themes += [theme]
        return themes

    def do_get_functions(self):
        out, err = run_fish_cmd("functions")
        out = out.strip()
        return out.split("\n")

    def do_get_variable_names(self, cmd):
        "Given a command like 'set -U' return all the variable names"
        out, err = run_fish_cmd(cmd)
        return out.split("\n")

    def do_get_variables(self):
        out, err = run_fish_cmd("set -L")

        # Put all the variables into a dictionary
        vars = {}
        for line in out.split("\n"):
            comps = line.split(" ", 1)
            if len(comps) < 2:
                continue
            fish_var = FishVar(comps[0], comps[1])
            vars[fish_var.name] = fish_var

        # Mark universal variables. L means don't abbreviate.
        for name in self.do_get_variable_names("set -nUL"):
            if name in vars:
                vars[name].universal = True
        # Mark exported variables. L means don't abbreviate.
        for name in self.do_get_variable_names("set -nxL"):
            if name in vars:
                vars[name].exported = True

        # Do not return history as a variable, it may be so large the browser hangs.
        vars.pop("history", None)

        return [
            vars[key].get_json_obj()
            for key in sorted(vars.keys(), key=lambda x: x.lower())
        ]

    def do_get_bindings(self):
        """Get key bindings"""

        # Running __fish_config_interactive print fish greeting and
        # loads key bindings
        greeting, err = run_fish_cmd(" __fish_config_interactive")

        # Load the key bindings and then list them with bind
        out, err = run_fish_cmd("__fish_config_interactive; bind")

        # Remove fish greeting from output
        out = out[len(greeting) :]

        # Put all the bindings into a list
        bindings = []
        command_to_binding = {}

        for line in out.split("\n"):
            comps = line.split(" ", 2)

            # If we don't have "bind", a sequence and a mapping,
            # it's not a valid binding.
            if len(comps) != 3:
                continue

            # Store the "--preset" value for later
            if comps[1] == "--preset":
                preset = True
                # There's possibly a way to do this faster, but it's not important.
                comps = line.split(" ", 3)[1:]
            elif comps[1] == "--user":
                preset = False
                comps = line.split(" ", 3)[1:]
            # Check again if we removed the level.
            if len(comps) != 3:
                continue

            key = comps[1]
            command = comps[2]

            if command in bindings_blacklist:
                continue

            if command in command_to_binding:
                fish_binding = command_to_binding[command]
                fish_binding.add_binding(line, key)
            else:
                fish_binding = FishBinding(command, line, key)
                bindings.append(fish_binding)
                command_to_binding[command] = fish_binding

        return [binding.get_json_obj() for binding in bindings]

    def do_get_history(self):
        # Use NUL to distinguish between history items.
        out, err = run_fish_cmd("builtin history -z")
        result = out.split("\0")
        if result:
            result.pop()  # trim off the trailing element
        return result

    def do_set_color_for_variable(self, name, color):
        "Sets a color for a fish color name, like 'fish_color_autosuggestion'"
        if not name:
            raise ValueError
        if not color and not color == "":
            color = "normal"
        else:
            color = unparse_color(color)
        name = name.strip()
        if not re.match("^[a-zA-Z0-9_]+$", name):
            print("Refusing to use variable name: '", name, "'")
            return
        color = color.strip()
        if not re.match("^[a-zA-Z0-9_= -]*$", color):
            print("Refusing to use color value: ", color)
            return
        return name + " " + color

    def do_get_function(self, func_name):
        out, err = run_fish_cmd(
            "functions -- " + escape_fish_cmd(func_name) + " | builtin fish_indent --html"
        )
        return out

    def do_delete_history_item(self, history_item_text):
        # It's really lame that we always return success here
        cmd = (
            "builtin history delete --case-sensitive --exact -- %s; builtin history save"
            % escape_fish_cmd(history_item_text)
        )
        out, err = run_fish_cmd(cmd)
        return True

    def do_set_prompt_function(self, prompt_func):
        cmd = (
            "functions -e fish_right_prompt; "
            + prompt_func
            + "\n"
            + "funcsave fish_prompt && funcsave fish_right_prompt 2>/dev/null"
        )
        out, err = run_fish_cmd(cmd)
        return len(err) == 0

    def do_get_prompt(self, prompt_function_text, extras_dict):
        # Return the prompt output by the given command
        cmd = (
            prompt_function_text
            + "\n builtin cd -- "
            + escape_fish_cmd(initial_wd)
            + " \n false \n fish_prompt\n"
        )
        prompt_demo_ansi, err = run_fish_cmd(cmd)
        prompt_demo_html = ansi_to_html(prompt_demo_ansi)
        right_demo_ansi, err = run_fish_cmd(
            "functions -e fish_right_prompt; "
            + prompt_function_text
            + "\n builtin cd -- "
            + escape_fish_cmd(initial_wd)
            + " \n false \n functions -q fish_right_prompt && fish_right_prompt\n"
        )
        right_demo_html = ansi_to_html(right_demo_ansi)
        prompt_demo_font_size = self.font_size_for_ansi_prompt(
            prompt_demo_ansi + right_demo_ansi
        )
        result = {
            "function": prompt_function_text,
            "demo": prompt_demo_html,
            "font_size": prompt_demo_font_size,
            "right": right_demo_html,
        }
        if extras_dict:
            result.update(extras_dict)
        return result

    def do_get_current_prompt(self):
        # Return the current prompt. We run 'false' to demonstrate how the
        # prompt shows the command status (#1624).
        prompt_func, err = run_fish_cmd(
            "functions fish_prompt; functions fish_right_prompt"
        )
        result = self.do_get_prompt(
            prompt_func.strip(),
            {"name": "Current"},
        )
        return result

    def do_get_sample_prompt(self, text, extras_dict):
        # Return the prompt you get from the given text. Extras_dict is a
        # dictionary whose values get merged in. We run 'false' to demonstrate
        # how the prompt shows the command status (#1624)
        return self.do_get_prompt(text.strip(), extras_dict)

    def parse_one_sample_prompt_hash(self, line, result_dict):
        # Allow us to skip whitespace, etc.
        if not line:
            return True
        if line.isspace():
            return True

        # Parse a comment hash like '# name: Classic'
        match = re.match(r"#\s*(\w+?): (.+)", line, re.IGNORECASE)
        if match:
            key = match.group(1).strip()
            value = match.group(2).strip()
            result_dict[key] = value
            return True
        # Skip other hash comments
        return line.startswith("#")

    def read_one_sample_prompt(self, path):
        try:
            extras_dict = {}
            # Read one sample prompt from out
            function_lines = []
            parsing_hashes = True
            for line in get_embedded_file(path).splitlines(keepends=True):
                # Parse hashes until parse_one_sample_prompt_hash return
                # False.
                if parsing_hashes:
                    parsing_hashes = self.parse_one_sample_prompt_hash(
                        line, extras_dict
                    )
                # Maybe not we're not parsing hashes, or maybe we already
                # were not.
                if not parsing_hashes:
                    function_lines.append(line)
            func = "".join(function_lines).strip()
            result = self.do_get_sample_prompt(func, extras_dict)
            return result
        except IOError:
            # Ignore unreadable files, etc.
            return None

    def do_get_sample_prompts_list(self):
        paths = list_embedded_files("prompts", ".fish")
        pool = None
        result = []
        try:
            pool = multiprocessing.pool.ThreadPool(processes=8)

            # Kick off the "Current" meta-sample
            current_metasample_async = pool.apply_async(self.do_get_current_prompt)

            # Read all of the prompts in sample_prompts
            sample_results = pool.map(self.read_one_sample_prompt, paths, 1)
            result.append(current_metasample_async.get())
            result.extend([r for r in sample_results if r])
        except ImportError:
            # If the platform doesn't support multiprocessing, we just do it one at a time.
            # This happens e.g. on Termux.
            print(
                "Platform doesn't support multiprocessing, running one at a time. This may take a while."
            )
            result.append(self.do_get_current_prompt())
            result.extend([self.read_one_sample_prompt(path) for path in paths])
        finally:
            if pool is not None:
                pool.close()
                pool.join()
        return result

    def secure_startswith(self, haystack, needle):
        if len(haystack) < len(needle):
            return False
        bits = 0
        for x, y in zip(haystack, needle):
            bits |= ord(x) ^ ord(y)
        return bits == 0

    def font_size_for_ansi_prompt(self, prompt_demo_ansi):
        width = ansi_prompt_line_width(prompt_demo_ansi)
        # Pick a font size
        if width >= 70:
            font_size = "8pt"
        if width >= 60:
            font_size = "10pt"
        elif width >= 50:
            font_size = "11pt"
        elif width >= 40:
            font_size = "13pt"
        elif width >= 30:
            font_size = "15pt"
        elif width >= 25:
            font_size = "16pt"
        elif width >= 20:
            font_size = "17pt"
        else:
            font_size = "18pt"
        return font_size

    def do_GET(self):
        p = self.path

        authpath = "/" + authkey
        if self.secure_startswith(p, authpath + "/"):
            p = p[len(authpath) :]
        else:
            return self.send_error(403)
        self.path = p

        if p == "/colors/":
            output = self.do_get_colors()
        elif p == "/functions/":
            output = self.do_get_functions()
        elif p == "/variables/":
            output = self.do_get_variables()
        elif p == "/history/":
            # start = time.time()
            output = self.do_get_history()
            # end = time.time()
            # print "History: ", end - start
        elif p == "/sample_prompts/":
            output = self.do_get_sample_prompts_list()
        elif p == "/bindings/":
            output = self.do_get_bindings()
        else:
            return SimpleHTTPServer.SimpleHTTPRequestHandler.do_GET(self)

        # Return valid output
        self.send_response(200)
        self.send_header("Content-type", "application/json")
        self.end_headers()
        self.write_to_wfile("\n")

        # Output JSON
        self.write_to_wfile(json.dumps(output))

    def reject_unauthenticated_post(self):
        # Closing a Windows socket with an unread request body can reset TCP
        # before the peer receives the 403. Discard only a small, framed body;
        # never decode it or pass it to a mutation handler.
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if 0 < length <= 64 * 1024 and not self.headers.get("Transfer-Encoding"):
            timeout = self.connection.gettimeout()
            deadline = time.monotonic() + (timeout if timeout is not None else 2)
            try:
                while length:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        break
                    self.connection.settimeout(remaining)
                    chunk = self.rfile.read1(min(length, 8192))
                    if not chunk:
                        break
                    length -= len(chunk)
            except TimeoutError:
                pass  # An unfinished rejected body does not extend the deadline.
            finally:
                self.connection.settimeout(timeout)
        self.send_error(403)

    def do_POST(self):
        p = self.path

        authpath = "/" + authkey
        if self.secure_startswith(p, authpath + "/"):
            p = p[len(authpath) :]
        else:
            return self.reject_unauthenticated_post()
        self.path = p

        # This is cheesy, we want just the actual content-type.
        # In some cases it'll give us the encoding as well,
        # ("application/json;charset=utf-8")
        # but we don't currently care.
        content_type = self.headers["content-type"].split(";")[0]

        if content_type == "application/x-www-form-urlencoded":
            length = int(self.headers["content-length"])
            url_str = self.rfile.read(length).decode("utf-8")
            postvars = parse_qs(url_str, keep_blank_values=1)
        elif content_type == "application/json":
            length = int(self.headers["content-length"])
            # This used to use the provided encoding, but we use utf-8
            # all around the place and nobody has ever complained.
            #
            # If any other encoding is received this will raise a UnicodeError,
            # which will throw us out of the function and should at most exit webconfig.
            # If that happens to anyone we expect bug reports.
            url_str = self.rfile.read(length).decode("utf-8")
            postvars = json.loads(url_str)
        elif content_type == "multipart/form-data":
            # This used to be a thing, as far as I could find there's
            # no use anymore, but let's keep an error around just in case.
            return self.send_error(500)
        else:
            postvars = {}

        if p == "/set_color/":
            print("# Colorscheme: " + postvars.get("theme"))
            output = ""
            theme_contents = []
            for item in postvars.get("colors"):
                what = item.get("what")
                color = item.get("color")
                if what:
                    theme_contents += [self.do_set_color_for_variable(what, color)]

            varname = "__fish_webconfig_theme_notification"
            out, err = run_fish_cmd(
                "__fish_theme_freeze 'fish_config' {}\n".format(
                    " ".join(escape_fish_cmd(x) for x in theme_contents)
                )
                + "begin\n"
                + "    set -l i 0\n"
                + "    if not set -q {}\n".format(varname)
                + "        or string match -qr -- '^set-theme-v1-#(?<i>\\d+)$' ${}\n".format(
                    varname
                )
                + "        set -U {} set-theme-v1-#(math $i + 1)\n".format(varname)
                + "    end\n"
                + "end\n"
            )
            assert out == "", out
            assert err == "", err

        elif p == "/get_function/":
            what = postvars.get("what")
            output = [self.do_get_function(what[0])]
        elif p == "/delete_history_item/":
            what = postvars.get("what")
            if self.do_delete_history_item(what[0]):
                output = ["OK"]
            else:
                output = ["Unable to delete history item"]
        elif p == "/set_prompt/":
            what = postvars.get("fish_prompt")
            if self.do_set_prompt_function(what):
                output = ["OK"]
            else:
                output = ["Unable to set prompt"]
        else:
            return self.send_error(404)

        # Return valid output
        self.send_response(200)
        self.send_header("Content-type", "application/json")
        self.end_headers()
        self.write_to_wfile("\n")

        # Output JSON
        self.write_to_wfile(json.dumps(output))

    def log_request(self, code="-", size="-"):
        """Disable request logging"""
        pass

    def log_error(self, format, *args):
        if format == "code %d, message %s" and hasattr(self, "path"):
            # This appears to be a send_error() message
            # We want to include the path (if we have one)
            (code, msg) = args
            format = "code %d, message %s, path %s"
            args = (code, msg, self.path)
        SimpleHTTPServer.SimpleHTTPRequestHandler.log_error(self, format, *args)


redirect_template_html = """
<!DOCTYPE html>
<html>
 <head>
  <meta http-equiv="refresh" content="0;URL='%s'" />
 </head>
 <body>
  <p><a href="%s">Start the Fish Web config</a></p>
 </body>
</html>
"""

def capture_enter(entered, stop):
    """Record ENTER/EOF without blocking cooperative cancellation."""
    try:
        if is_windows():
            import ctypes
            import msvcrt

            handle = msvcrt.get_osfhandle(sys.stdin.fileno())
            kernel = ctypes.WinDLL("kernel32", use_last_error=True)
            kernel.GetFileType.argtypes = [ctypes.c_void_p]
            kernel.PeekNamedPipe.argtypes = [
                ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32,
                ctypes.c_void_p, ctypes.POINTER(ctypes.c_uint32), ctypes.c_void_p,
            ]
            kind = kernel.GetFileType(handle)
            while not stop.is_set():
                if kind == 2:  # FILE_TYPE_CHAR: never block in getwch.
                    if msvcrt.kbhit() and msvcrt.getwch() in ("\r", "\n"):
                        break
                elif kind == 3:  # FILE_TYPE_PIPE
                    available = ctypes.c_uint32()
                    if not kernel.PeekNamedPipe(handle, None, 0, None, ctypes.byref(available), None):
                        break  # The pipe writer closed its owned end.
                    if available.value and os.read(sys.stdin.fileno(), 1) in (b"\r", b"\n", b""):
                        break
                elif os.read(sys.stdin.fileno(), 1) in (b"\r", b"\n", b""):
                    break
                stop.wait(0.05)
        else:
            while not stop.is_set():
                if select.select([sys.stdin], [], [], 0.1)[0]:
                    if os.read(sys.stdin.fileno(), 1) in (b"\r", b"\n", b""):
                        break
        if not stop.is_set():
            entered.set()
    except (OSError, ValueError):
        if not stop.is_set():
            entered.set()


def windows_browser_argv(command, fileurl):
    """Parse Windows command-line conventions, not POSIX sh or colon lists."""
    import ctypes

    count = ctypes.c_int()
    shell = ctypes.WinDLL("shell32", use_last_error=True)
    shell.CommandLineToArgvW.argtypes = [ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_int)]
    shell.CommandLineToArgvW.restype = ctypes.POINTER(ctypes.c_wchar_p)
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.LocalFree.argtypes = [ctypes.c_void_p]
    kernel.LocalFree.restype = ctypes.c_void_p
    pointer = shell.CommandLineToArgvW(command, ctypes.byref(count))
    if not pointer:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        argv = [pointer[index] for index in range(count.value)]
    finally:
        kernel.LocalFree(ctypes.cast(pointer, ctypes.c_void_p))
    if not argv or os.path.splitext(argv[0])[1].lower() in (".cmd", ".bat"):
        raise OSError("BROWSER must name a native executable, not a command-shell script")
    if any("%s" in argument for argument in argv):
        return [argument.replace("%s", fileurl) for argument in argv]
    return argv + [fileurl]


def launch_browser(fileurl, url):
    if os.environ.get("BROWSER") == "true":
        return True
    if is_windows():
        if os.environ.get("BROWSER"):
            return subprocess.Popen(windows_browser_argv(os.environ["BROWSER"], fileurl))
        # ShellExecute opens one literal URL and returns without a browser worker.
        os.startfile(fileurl)
        return True
    if is_wsl():
        cmd = find_executable("cmd.exe", COMMON_WSL_CMD_PATHS)
        if not cmd:
            return False
        return subprocess.call([cmd, "/c", "start", "", fileurl]) == 0
    if is_termux():
        return subprocess.call(["termux-open-url", url]) == 0
    if is_sailfish_os():
        return subprocess.call(["xdg-open", url]) == 0
    return webbrowser.open(url if is_chromeos_garcon() else fileurl)


def main():
    global FISH_BIN_PATH, initial_wd, authkey
    if not (3, 8) <= sys.version_info < (4, 0):
        print("fish_config browse requires Python 3.8 or newer.", file=sys.stderr)
        return 1
    fish_name = "fish.exe" if is_windows() else "fish"
    # fish_config supplies its actual image path, never a different installation
    # found earlier on PATH. Direct invocation can explicitly select an image.
    FISH_BIN_PATH = os.environ.get("fish_bin_path")
    if not FISH_BIN_PATH:
        bin_dir = os.environ.get("fish_bin_dir")
        FISH_BIN_PATH = os.path.join(bin_dir, fish_name) if bin_dir else find_executable(fish_name)
    if not FISH_BIN_PATH or not os.access(FISH_BIN_PATH, os.X_OK):
        print("fish could not be executed; set fish_bin_path to the matching fish installation.", file=sys.stderr)
        return 1
    FISH_BIN_PATH = os.path.abspath(FISH_BIN_PATH)
    initial_wd = os.getcwd()
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    authkey = binascii.b2a_hex(os.urandom(16)).decode("ascii")
    # Bind only loopback; verify_request remains a second independent boundary.
    host = "::1" if socket.has_ipv6 else "127.0.0.1"
    server = None
    redirect = None
    input_thread = None
    browser = None
    previous_break = None
    if is_windows():
        def interrupted(signum, frame):
            raise KeyboardInterrupt
        previous_break = signal.signal(signal.SIGBREAK, interrupted)
    stop = threading.Event()
    entered = threading.Event()
    try:
        for port in range(8000, 9001):
            try:
                server = FishConfigTCPServer((host, port), FishConfigHTTPRequestHandler)
                break
            except OSError as error:
                if error.errno in (errno.EAFNOSUPPORT, errno.EADDRNOTAVAIL) and host == "::1":
                    host = "127.0.0.1"
                    FishConfigTCPServer.address_family = socket.AF_INET
                    server = FishConfigTCPServer((host, port), FishConfigHTTPRequestHandler)
                    break
                if error.errno != errno.EADDRINUSE:
                    raise
        if server is None:
            raise OSError("Unable to find an open port between 8000 and 9000")
        initial_tab = ""
        if len(sys.argv) > 1:
            for tab in ("functions", "prompt", "colors", "variables", "history", "bindings"):
                if tab.startswith(sys.argv[1]):
                    initial_tab = "#!/" + tab
                    break
        url_host = "[::1]" if host == "::1" else "127.0.0.1"
        url = "http://%s:%d/%s/%s" % (url_host, server.server_address[1], authkey, initial_tab)
        # TemporaryDirectory is private and works on Python 3.8+ on Windows.
        # Close the redirect before opening it; delete only after service shutdown.
        redirect = tempfile.TemporaryDirectory(prefix="fish-webconfig-")
        redirect_path = Path(redirect.name) / "redirect.html"
        redirect_path.write_text(redirect_template_html % (url, url), encoding="utf-8")
        fileurl = redirect_path.as_uri()
        print("Web config started at %s%s%s" % (ENTER_UNDERLINE_MODE, fileurl, EXIT_ATTRIBUTE_MODE))
        print("If that doesn't work, try opening %s%s%s" % (ENTER_UNDERLINE_MODE, url, EXIT_ATTRIBUTE_MODE))
        print("%sHit ENTER to stop.%s" % (ENTER_BOLD_MODE, EXIT_ATTRIBUTE_MODE), flush=True)
        browser = launch_browser(fileurl, url)
        if not browser:
            raise OSError("Unable to launch a browser; set BROWSER or open the displayed URL.")
        input_thread = threading.Thread(target=capture_enter, args=(entered, stop), name="fish-config-input")
        input_thread.start()
        while True:
            # The input event needs no auxiliary TCP connection. In particular,
            # Windows socketpair() may itself block establishing loopback TCP.
            ready, _, _ = select.select([server], [], [], 0.1)
            if isinstance(browser, subprocess.Popen) and browser.poll() not in (None, 0):
                raise OSError("Browser exited with status %d" % browser.returncode)
            if entered.is_set():
                print("Shutting down.")
                return 0
            if server in ready:
                server.handle_request()
    except KeyboardInterrupt:
        print("\nShutting down.")
        return 0
    except OSError as error:
        print("fish_config: %s" % error, file=sys.stderr)
        return 1
    finally:
        stop.set()
        if isinstance(browser, subprocess.Popen):
            if browser.poll() is None:
                browser.terminate()
            try:
                browser.wait(timeout=5)
            except subprocess.TimeoutExpired:
                browser.kill()
                browser.wait()
        if input_thread is not None:
            input_thread.join()
        if server is not None:
            server.server_close()
        if redirect is not None:
            redirect.cleanup()
        os.chdir(initial_wd)
        if previous_break is not None:
            signal.signal(signal.SIGBREAK, previous_break)


if __name__ == "__main__":
    sys.exit(main())
