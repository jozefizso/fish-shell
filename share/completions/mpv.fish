if command -sq mpv
    set -l options (command mpv --list-options 2>/dev/null)
    set -l names (string replace -fr '^\s*--([\w-]+).*' '$1' -- $options)
    set -l flags (string replace -fr '^\s*--([\w-]+).*Flag.*' '$1' -- $options)
    if set -q names[1]
        complete -c mpv -l$names
    end
    if set -q flags[1]
        complete -c mpv -lno-$flags
    end
end

complete -c mpv -l start -x -d "Seek to given position (%, s, hh:mm:ss)"
complete -c mpv -l no-audio -d "Disable audio"
complete -c mpv -l no-audio-display -d "Hide attached picture for audio"
complete -c mpv -l no-video -d "Disable video"
complete -c mpv -l fs -d "Fullscreen playback"
complete -c mpv -l sub-file -r -d "Specify subtitle file"
complete -c mpv -l playlist -r -d "Specify playlist file"
