# localization: skip(private)
function __fish_complete_zfs_pools -d "Completes with available ZFS pools"
    command -sq zpool; or return 1
    zpool list -o name,comment -H | string replace -a \t'-' ''
end
