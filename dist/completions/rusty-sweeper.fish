# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_rusty_sweeper_global_optspecs
	string join \n c/config= v/verbose q/quiet h/help V/version
end

function __fish_rusty_sweeper_needs_command
	# Figure out if the current invocation already has a command.
	set -l cmd (commandline -opc)
	set -e cmd[1]
	argparse -s (__fish_rusty_sweeper_global_optspecs) -- $cmd 2>/dev/null
	or return
	if set -q argv[1]
		# Also print the command, so this can be used to figure out what it is.
		echo $argv[1]
		return 1
	end
	return 0
end

function __fish_rusty_sweeper_using_subcommand
	set -l cmd (__fish_rusty_sweeper_needs_command)
	test -z "$cmd"
	and return 1
	contains -- $cmd[1] $argv
end

complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -s c -l config -d 'Path to configuration file' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -s v -l verbose -d 'Increase verbosity (-v, -vv, -vvv)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -s q -l quiet -d 'Suppress non-essential output'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -s h -l help -d 'Print help'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -s V -l version -d 'Print version'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -f -a "monitor" -d 'Start disk usage monitoring'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -f -a "clean" -d 'Scan for projects and clean build artifacts'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -f -a "scan" -d 'Analyze disk usage of a directory'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -f -a "tui" -d 'Launch interactive TUI'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -f -a "completions" -d 'Generate shell completions'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s i -l interval -d 'Check interval in seconds' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s w -l warn -d 'Warning threshold percentage' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s C -l critical -d 'Critical threshold percentage' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s m -l mount -d 'Mount points to monitor (can be specified multiple times)' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -l notify -d 'Notification backend (auto, dbus, notify-send, stderr)' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s c -l config -d 'Path to configuration file' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s d -l daemon -d 'Run as background daemon'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -l once -d 'Check once and exit'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -l stop -d 'Stop running daemon'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -l status -d 'Show daemon status'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s v -l verbose -d 'Increase verbosity (-v, -vv, -vvv)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s q -l quiet -d 'Suppress non-essential output'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s h -l help -d 'Print help'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand monitor" -s V -l version -d 'Print version'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s d -l max-depth -d 'Maximum recursion depth' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s t -l types -d 'Project types to clean (comma-separated)' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s e -l exclude -d 'Paths to exclude (glob patterns)' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s a -l age -d 'Only clean projects not modified in N days' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s j -l jobs -d 'Parallel clean jobs' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s c -l config -d 'Path to configuration file' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s n -l dry-run -d 'Show what would be cleaned without doing it'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s f -l force -d 'Skip confirmation prompts'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -l size-only -d 'Only report sizes, don\'t clean'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s v -l verbose -d 'Increase verbosity (-v, -vv, -vvv)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s q -l quiet -d 'Suppress non-essential output'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s h -l help -d 'Print help'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand clean" -s V -l version -d 'Print version'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s d -l max-depth -d 'Maximum depth to display' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s n -l top -d 'Show top N entries by size' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s j -l jobs -d 'Parallel scan threads' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -l sort -d 'Sort by: size, name, mtime' -r
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s c -l config -d 'Path to configuration file' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s a -l all -d 'Include hidden files'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s x -l one-file-system -d 'Don\'t cross filesystem boundaries'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -l json -d 'Output as JSON'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s v -l verbose -d 'Increase verbosity (-v, -vv, -vvv)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s q -l quiet -d 'Suppress non-essential output'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s h -l help -d 'Print help'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand scan" -s V -l version -d 'Print version'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -s c -l config -d 'Path to configuration file' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -s x -l one-file-system -d 'Don\'t cross filesystem boundaries'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -l no-color -d 'Disable colors'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -s v -l verbose -d 'Increase verbosity (-v, -vv, -vvv)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -s q -l quiet -d 'Suppress non-essential output'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -s h -l help -d 'Print help'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand tui" -s V -l version -d 'Print version'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand completions" -s c -l config -d 'Path to configuration file' -r -F
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand completions" -s v -l verbose -d 'Increase verbosity (-v, -vv, -vvv)'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand completions" -s q -l quiet -d 'Suppress non-essential output'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand completions" -s h -l help -d 'Print help'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand completions" -s V -l version -d 'Print version'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand help; and not __fish_seen_subcommand_from monitor clean scan tui completions help" -f -a "monitor" -d 'Start disk usage monitoring'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand help; and not __fish_seen_subcommand_from monitor clean scan tui completions help" -f -a "clean" -d 'Scan for projects and clean build artifacts'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand help; and not __fish_seen_subcommand_from monitor clean scan tui completions help" -f -a "scan" -d 'Analyze disk usage of a directory'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand help; and not __fish_seen_subcommand_from monitor clean scan tui completions help" -f -a "tui" -d 'Launch interactive TUI'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand help; and not __fish_seen_subcommand_from monitor clean scan tui completions help" -f -a "completions" -d 'Generate shell completions'
complete -c rusty-sweeper -n "__fish_rusty_sweeper_using_subcommand help; and not __fish_seen_subcommand_from monitor clean scan tui completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
