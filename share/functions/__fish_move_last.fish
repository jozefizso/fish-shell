# localization: skip(private)
function __fish_move_last -d "Move the last element of a directory history from src to dest"
    set -l src $argv[1]
    set -l dest $argv[2]

    set -l size_src (count $$src)

    if test $size_src = 0
        # Cannot make this step
        # Print a bel, which is the character to print for notifications like these.
        printf \a
        return 1
    end

    if builtin -q __fish_native_fs
        # A removed or denied history directory is a real failed action. Do not
        # consume history or advertise success when native cd did not happen.
        set -l previous $PWD
        set -l source $$src
        builtin cd $source[$size_src]
        or return
        set -g $dest $$dest $previous
        set -e $src\[$size_src]
        return 0
    end
    # Append current dir to the end of the destination
    set -g (echo $dest) $$dest $PWD

    set -l ssrc $$src

    # Change dir to the last entry in the source dir-hist
    builtin cd $ssrc[$size_src]

    # Keep all but the last from the source dir-hist
    set -e (echo $src)\[$size_src]

    # All ok, return success
    return 0
end
