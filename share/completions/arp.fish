#completion for arp
if __fish_is_windows_command arp; or begin; builtin -q __fish_native_fs; and not command -sq arp; end
    complete -c arp -s a -s g -d 'Display ARP entries'
    complete -c arp -s v -d 'Include invalid and loopback entries'
    complete -c arp -s N -xa '(__fish_print_addresses | string match -r "^[0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+\\t.*")' -d 'Select interface by IPv4 address'
    complete -c arp -s d -x -d 'Delete an ARP entry'
    complete -c arp -s s -x -d 'Add a static ARP entry'
    complete -c arp -a '/?' -d 'Show help'
    return
end

switch (__fish_uname)
    case Darwin
        complete -c arp -s n -d 'Show network addresses as numbers'
        complete -c arp -s i -x -a "(__fish_print_interfaces)" -d 'Limit scope to interface'
        complete -c arp -s l -d 'List entries in long format'
        complete -c arp -s a -d 'Display all ARP entries'
        complete -c arp -s d -x -a "(__fish_print_hostnames)" -d 'Delete an ARP entry'
        complete -c arp -s s -x -a "(__fish_print_hostnames)" -d 'Set an ARP entry'
        complete -c arp -s S -x -a "(__fish_print_hostnames)" -d 'Set a static ARP entry'
        complete -c arp -s f -r -d 'Read entries from file'
    case '*'
        complete -c arp -s v -l verbose -d "Verbose mode"
        complete -c arp -s n -l numeric -d "Numerical address"
        complete -x -c arp -s H -l tw-type -a "ether arcnet pronet ax25 netrom" -d "Class of hw type"
        complete -c arp -s a -l display -x -a "(__fish_print_hostnames)" -d "Show arp entries"
        complete -x -c arp -s d -l delete -a "(__fish_print_hostnames)" -d "Remove an entry for hostname"
        complete -c arp -s D -l use-device -d "Use hardware address"
        complete -x -c arp -s i -l device -a "(__fish_print_interfaces)" -d "Select interface"
        complete -x -c arp -s s -l set -d "Manually create ARP address" -a "(__fish_print_hostnames)"
        complete -f -c arp -s f -l file -d "Take addr from filename, default /etc/ethers"
end
