fish_update_completions - update completions using manual pages
===============================================================

Synopsis
--------

.. synopsis::

    fish_update_completions [--directory DIRECTORY] [--keep] [MANPAGE ...]

Description
-----------

``fish_update_completions`` parses manual pages and writes generated completion files to fish's generated-completion cache. It requires Python 3.8 or newer, but Python is not a shell-startup dependency.

This does not overwrite custom completions.

Explicit *MANPAGE* arguments can name plain roff pages or pages compressed with gzip, bzip2, xz or lzma. Xz and lzma require Python's optional ``lzma`` module. Unreadable or corrupt pages produce a diagnostic and a nonzero status; successfully generated files are published atomically. ``--directory DIRECTORY`` selects an output directory, and ``--keep`` preserves existing generated completions instead of refreshing the directory.

On native Windows there is no assumed system-wide inventory of Unix manuals. Supply explicit files, or set ``MANPATH`` to a semicolon-separated list of manual roots containing ``man1``, ``man6`` or ``man8`` directories. Drive prefixes and Unicode paths are preserved. Without configured manuals, the tool explains how to supply them; ordinary startup does not invoke it.

Setting ``fish_update_completions_detach=true`` starts background generation. Native Windows uses fish's native background-process ownership, not a POSIX shell. The child receives a private copy of the embedded generator and removes it when finished, so generation can outlive the initiating shell without retaining its stdin bridge.
