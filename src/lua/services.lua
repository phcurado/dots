dots.provider.service("systemd", {
	started = 'systemctl is-active --quiet "$DOTS_SERVICE"',
	start = 'sudo systemctl start "$DOTS_SERVICE"',
	stop = 'sudo systemctl stop "$DOTS_SERVICE"',
	enabled = 'systemctl is-enabled --quiet "$DOTS_SERVICE"',
	enable = 'sudo systemctl enable "$DOTS_SERVICE"',
	disable = 'sudo systemctl disable "$DOTS_SERVICE"',
})

dots.provider.service("brew-service", {
	started = [[brew services list | awk -v service="$DOTS_SERVICE" '$1 == service && $2 == "started" { found = 1 } END { exit !found }]],
	start = 'brew services start "$DOTS_SERVICE"',
	stop = 'brew services stop "$DOTS_SERVICE"',
})

dots.brew.service = {
	start = dots["brew-service"].start,
}
