`fish <https://fishshell.com/>`__ - the friendly interactive shell |Build Status|
=================================================================================

fish is a smart and user-friendly command line shell for macOS, Linux,
and the rest of the family. fish includes features like syntax
highlighting, autosuggest-as-you-type, and fancy tab completions that
just work, with no configuration required.

For downloads, screenshots and more, go to https://fishshell.com/.

Quick Start
-----------

fish generally works like other shells, like bash or zsh. A few
important differences can be found at
https://fishshell.com/docs/current/tutorial.html by searching for the
magic phrase “unlike other shells”.

Detailed user documentation is available by running ``help`` within
fish, and also at https://fishshell.com/docs/current/index.html

Getting fish
------------

macOS
~~~~~

fish can be installed:

-  using `Homebrew <https://brew.sh/>`__: ``brew install fish``
-  using `MacPorts <https://www.macports.org/>`__:
   ``sudo port install fish``
-  using the `installer from fishshell.com <https://fishshell.com/>`__
-  as a `standalone app from fishshell.com <https://fishshell.com/>`__

Note: The minimum supported macOS version is 10.12.

Packages for Linux
~~~~~~~~~~~~~~~~~~

Packages for Debian, Fedora, openSUSE, and Red Hat Enterprise
Linux/CentOS are available from the `openSUSE Build
Service <https://software.opensuse.org/download.html?project=shells%3Afish&package=fish>`__.

Packages for Ubuntu are available from the `fish
PPA <https://launchpad.net/~fish-shell/+archive/ubuntu/release-4>`__,
and can be installed using the following commands:

::

   sudo apt-add-repository ppa:fish-shell/release-4
   sudo apt update
   sudo apt install fish

Instructions for other distributions may be found at
`fishshell.com <https://fishshell.com>`__.

Windows
~~~~~~~

The native x64 MSVC distribution path is a **relocatable, unsigned ZIP**, separate
from WSL/Cygwin/MSYS2. Build and installation commands are below. Its currently
declared test baseline is x64 Windows build 10.0.26300, local NTFS, Windows SDK
10.0.26100.0, the native console/ConPTY transport and Rust 1.85/current stable.
This is not certification of older Windows builds, ARM64, FAT/ReFS, remote SMB
globbing, MSI/MSIX/winget or signed publication. Package/gate implementation is
not a clean-machine or human-signoff result; consult the recorded gate evidence
before advertising a release.

-  On Windows 10/11, fish can be installed under the WSL Windows Subsystem
   for Linux with the instructions for the appropriate distribution
   listed above under “Packages for Linux”, or from source with the
   instructions below.
-  fish can also be installed on all versions of Windows using
   `Cygwin <https://cygwin.com/>`__ or `MSYS2 <https://github.com/Berrysoft/fish-msys2>`__.

Building from source
~~~~~~~~~~~~~~~~~~~~

If packages are not available for your platform, GPG-signed tarballs are
available from `fishshell.com <https://fishshell.com/>`__ and
`fish-shell on
GitHub <https://github.com/fish-shell/fish-shell/releases>`__. See the
`Building <#building>`_ section for instructions.

Running fish
------------

Once installed, run ``fish`` from your current shell to try fish out!

Dependencies
~~~~~~~~~~~~

Running fish requires:

-  some common \*nix system utilities (currently ``mktemp``), in
   addition to the basic POSIX utilities (``cat``, ``cut``, ``dirname``,
   ``ls``, ``mkdir``, ``mkfifo``, ``rm``, ``sh``, ``sort``, ``tee``, ``tr``,
   ``uname`` and ``sed`` at least, but the full coreutils plus ``find`` and
   ``awk`` is preferred)

The following optional features also have specific requirements:

-  builtin commands that have the ``--help`` option or print usage
   messages require ``man`` for display
-  automated completion generation from manual pages requires Python 3.5+
-  the ``fish_config`` web configuration tool requires Python 3.5+ and a web browser
-  the :ref:`alt-o <shared-binds-alt-o>` binding requires the ``file`` program.
-  system clipboard integration (with the default Ctrl-V and Ctrl-X
   bindings) require either the ``xsel``, ``xclip``,
   ``wl-copy``/``wl-paste`` or ``pbcopy``/``pbpaste`` utilities
-  full completions for ``yarn`` and ``npm`` require the
   ``all-the-package-names`` NPM module
-  ``colorls`` is used, if installed, to add color when running ``ls`` on platforms
   that do not have color support (such as OpenBSD)

Building
--------

Dependencies
~~~~~~~~~~~~

Compiling fish requires:

-  Rust (version 1.85 or later), including cargo
-  CMake (version 3.15 or later)
-  a C compiler (for system feature detection and the test helper binary)
-  PCRE2 (headers and libraries) - optional, this will be downloaded if missing
-  gettext (only the msgfmt tool) - optional, for translation support
-  an Internet connection, as other dependencies will be downloaded automatically

Sphinx is also optionally required to build the documentation from a
cloned git repository.

Additionally, running the full test suite requires diff, git, Python 3.5+, pexpect, less, tmux and wget.

Building from source with CMake
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Rather than building from source, consider using a packaged build for your platform. Using the
steps below makes fish difficult to uninstall or upgrade. Release packages are available from the
links above, and up-to-date `development builds of fish are available for many platforms
<https://github.com/fish-shell/fish-shell/wiki/Development-builds>`__

To install into ``/usr/local``, run:

.. code:: shell

   mkdir build; cd build
   cmake ..
   cmake --build .
   sudo cmake --install .

The install directory can be changed using the
``-DCMAKE_INSTALL_PREFIX`` parameter for ``cmake``.

CMake Build options
~~~~~~~~~~~~~~~~~~~

In addition to the normal CMake build options (like ``CMAKE_INSTALL_PREFIX``), fish's CMake build has some other options available to customize it.

- Rust_COMPILER=path - the path to rustc. If not set, cmake will check $PATH and ~/.cargo/bin
- Rust_CARGO=path - the path to cargo. If not set, cmake will check $PATH and ~/.cargo/bin
- Rust_CARGO_TARGET=target - the target to pass to cargo. Set this for cross-compilation.
- WITH_DOCS=ON|OFF - whether to build the documentation. By default, this is ON when Sphinx is installed.
- FISH_INDENT_FOR_BUILDING_DOCS - useful for cross-compilation.
  Set this to the path to the ``fish_indent`` executable to use for building HTML docs.
  By default, ``${CMAKE_BINARY_DIR}/fish_indent`` will be used.
  If that's not runnable on the compile host,
  you can build a native one with ``cargo build --bin fish_indent`` and set this to ``$PWD/target/debug/fish_indent``.
- FISH_USE_SYSTEM_PCRE2=ON|OFF - whether to use an installed pcre2. This is normally autodetected.
- WITH_MESSAGE_LOCALIZATION=ON|OFF - whether to include translations.
- extra_functionsdir, extra_completionsdir and extra_confdir - to compile in an additional directory to be searched for functions, completions and configuration snippets

Building fish with Cargo
~~~~~~~~~~~~~~~~~~~~~~~~

You can also build fish with Cargo.
This example uses `uv <https://github.com/astral-sh/uv>`__ to install Sphinx (which is used for man-pages and ``--help`` options).
You can also install Sphinx another way and drop the ``uv run --no-managed-python`` prefix.

.. code:: shell

    git clone https://github.com/fish-shell/fish-shell
    cd fish-shell

    # Optional: check out a specific version rather than building the latest
    # development version.
    git checkout "$(git for-each-ref refs/tags/ | awk '$2 == "tag" { print $3 }' | tail -1)"

    uv run --no-managed-python \
        cargo install --locked --path .

This will place standalone binaries in ``~/.cargo/bin/``, but you can move them wherever you want.

To disable translations, disable the ``localize-messages`` feature by passing ``--no-default-features --features=embed-manpages`` to cargo.

You can also link this build statically (but not against glibc) and move it to other computers.

Here are the remaining advantages of a full installation, as currently done by CMake:

- Man pages like ``fish(1)`` installed in standard locations, easily accessible from outside fish.
- Separate files for builtins (e.g. ``$PREFIX/share/fish/man/man1/abbr.1``).
- A local copy of the HTML documentation, typically accessed via the ``help`` fish function.
  In Cargo builds, ``help`` will redirect to `<https://fishshell.com/docs/current/>`__
- Ability to use our CMake options extra_functionsdir, extra_completionsdir and extra_confdir,
  (also recorded in ``$PREFIX/share/pkgconfig/fish.pc``)
  which are used by some package managers to house third-party completions.
  Regardless of build system, fish uses ``$XDG_DATA_DIRS/{vendor_completion.d,vendor_conf.d,vendor_functions.d}``.

Native Windows build, ZIP and installation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Use native PowerShell 7.4+ in an x64 Visual Studio Developer environment with
MSVC C/C++ tools and Windows SDK 10.0.26100.0 (including ``rc.exe`` and
``dumpbin.exe``). Rust/Cargo must use ``x86_64-pc-windows-msvc``. Native builds do
not invoke CMake, ``sh``, WSL or MSYS. Select the SDK explicitly when initializing
``VsDevCmd.bat -arch=x64 -host_arch=x64 -winsdk=10.0.26100.0``.

The ``default`` profile preserves Cargo's ``embed-manpages`` and
``localize-messages`` features: install native Sphinx (and the Python dependencies
in ``pyproject.toml``) plus GNU gettext ``msgfmt`` on the **build host**.
``minimal`` explicitly disables default features and uses English messages.
Both ZIP profiles install version-matched text help, so packaging either profile
requires native ``sphinx-build``. Missing requested tools or failing generators
are errors, not permission to strip features. Python/Sphinx/gettext and Rust
are not shell-startup dependencies. Optional Python user tools require Python
3.8+ only when invoked.

.. code:: powershell

    pwsh -NoProfile -File build_tools/native.ps1 -Action Build -Configuration debug -Features default
    pwsh -NoProfile -File build_tools/native.ps1 -Action Build -Configuration release -Features default
    pwsh -NoProfile -File build_tools/native.ps1 -Action Package -Features default -OutputDirectory D:\fish-artifacts
    # For an explicitly English, nonembedded-help runtime:
    pwsh -NoProfile -File build_tools/native.ps1 -Action Package -Features minimal -OutputDirectory D:\fish-minimal-artifacts

The package contains three real binaries under ``bin``, installed resources under
``share/fish``, system configuration under ``etc/fish`` and help/licenses/notices
under ``share/doc/fish``. Fluent/gettext catalogs are embedded in the default
profile; no gettext runtime DLL is needed. Static MSVC CRT and static UTF-32
PCRE2 are selected, and packaging rejects undeployed non-system DLL imports.
Supported Windows supplies the remaining Windows/UCRT API DLLs. Version resources,
the lockfile hash, build inputs, import lists, per-file hashes and a ZIP SHA-256
sidecar accompany the unsigned artifact.
ZIP distribution is release-only; ``Build``/``Check`` also support debug binaries
for local testing, not redistribution of debug CRTs. Build release packages with
a redistribution-permitted MSVC toolchain under its applicable license terms.

Download the chosen ZIP and verify its SHA-256 against the independently obtained
release checksum before running its included ``native.ps1``. Extract it into a
temporary folder with Explorer's **Extract All**, then use the included script
from that folder (supply the ZIP's actual absolute path below). Alternatively use
the matching trusted source-checkout script. Extraction/install is user-local:

.. code:: powershell

    $versions = Join-Path $env:LOCALAPPDATA 'Programs\fish'
    New-Item -ItemType Directory -Force $versions | Out-Null
    $zip = (Resolve-Path 'C:\Users\<you>\Downloads\fish-<version>-windows-x64-default-unsigned.zip').Path
    Get-FileHash -Algorithm SHA256 $zip
    $install = Join-Path $versions '<version>'
    pwsh -NoProfile -File .\native.ps1 -Action Install -Archive $zip -Destination $install
    & (Join-Path $install 'bin\fish.exe') -c 'echo installed; string match -r fish fish'
    & (Join-Path $install 'bin\fish_indent.exe') --version
    & (Join-Path $install 'bin\fish_key_reader.exe') --help
    # Optional, current PowerShell session only; no profile or registry writes:
    $env:PATH = (Join-Path $install 'bin') + [IO.Path]::PathSeparator + $env:PATH

For persistent PATH selection, use Windows Settings' user environment-variable
editor and add the selected version's ``bin`` directory yourself. For an optional
Windows Terminal profile, use **Add a new profile**, give it a distinct name, and
set its command line to the quoted absolute ``...\bin\fish.exe`` path. Do not
overwrite another profile or make it the default unless that is your own choice.
No script here edits PATH, Terminal settings, login-shell settings or profiles.

Upgrade by installing to a **new version directory**, smoke-test it, and explicitly
select its PATH/Terminal entry. Existing destinations are never overwritten.
Relocate by closing that version's processes and moving its entire directory;
update only your selected PATH/Terminal entry. Remove an old version with:

.. code:: powershell

    pwsh -NoProfile -File .\native.ps1 -Action Remove -Destination $oldVersionDirectory

Removal validates tracked contents, rejects modified/untracked files and active
locks, and never touches independent config/history/universal-variable roots.
Preserve those roots during upgrades/removal: explicit XDG/HOME overrides take
precedence; the native default is the existing LocalAppData fish state policy.
Locked, denied, corrupt and partial operations return nonzero. A removal race can
leave an explicitly named ``.removing-*`` package directory; the diagnostic names
it rather than falsely reporting success. Close owned processes, resolve the
reported access problem and inspect the remainder before manually removing it.
Neither removal nor reinstall is a user-data reset.

``-Action Check`` runs native unit/doctest/integration/workspace-runtime suites,
rejecting zero/ignored mandatory suites. It requires the documented native
console/UNC/security fixtures plus the N10 ZIP, prior-version ZIP and clean
restricted-user fixture; see ``doc_internal/win_compat/15-build-packaging-documentation.md``.
Native Fluent rewriting/extraction and HTML generation have separate development
host evidence. HTML generation selects a matching host ``fish_indent`` directly,
without a Unix symlink or PATH separator convention. Unix-specific ``cargo xtask``
check/release commands remain separate. None of these development checks replaces
native installed-product, clean-machine or human accessibility acceptance.

Contributing Changes to the Code
--------------------------------

See the `Guide for Developers <CONTRIBUTING.rst>`__.

Contact Us
----------

Questions, comments, rants and raves can be posted to the official fish
mailing list at https://lists.sourceforge.net/lists/listinfo/fish-users
or join us on our `matrix
channel <https://matrix.to/#/#fish-shell:matrix.org>`__. Or use the `fish tag
on Unix & Linux Stackexchange <https://unix.stackexchange.com/questions/tagged/fish>`__.
There is also a fish tag on Stackoverflow, but it is typically a poor fit.

Found a bug? Have an awesome idea? Please `open an
issue <https://github.com/fish-shell/fish-shell/issues/new>`__.

.. |Build Status| image:: https://github.com/fish-shell/fish-shell/actions/workflows/test.yml/badge.svg
   :target: https://github.com/fish-shell/fish-shell/actions/workflows/test.yml
