function fish_clipboard_paste
    set -l data
    if builtin -q __fish_native_fs
        # Preserve trailing newlines; the provider already converts CRLF to LF.
        # Keep the helper's existing empty=1 policy for both editor and stream
        # paste. __fish_paste owns escaping and multiline insertion below.
        set data (__fish_clipboard paste | string collect -N)
        or return 1
    else if type -q pbpaste
        set data (pbpaste 2>/dev/null | string collect -N)
    else if set -q WAYLAND_DISPLAY; and type -q wl-paste
        set data (wl-paste -n 2>/dev/null | string collect -N)
    else if set -q DISPLAY; and type -q xsel
        set data (xsel --clipboard | string collect -N)
    else if set -q DISPLAY; and type -q xclip
        set data (xclip -selection clipboard -o 2>/dev/null | string collect -N)
    else if type -q powershell.exe
        # Preserve the Unix/WSL integration; native Windows uses the builtin.
        set data (powershell.exe Get-Clipboard | string trim -r -c \r | string collect -N)
    end

    # Issue 6254: Handle zero-length clipboard content
    if not string length -q -- "$data"
        return 1
    end

    if not isatty stdout
        # If we're redirected, just write the data *as-is*.
        printf %s $data
        return
    end

    __fish_paste $data
end
