dots.provider.service("systemd", {
	started = 'systemctl is-active --quiet "$DOTS_SERVICE"',
	start = 'sudo systemctl start "$DOTS_SERVICE"',
	stop = 'sudo systemctl stop "$DOTS_SERVICE"',
	enabled = 'systemctl is-enabled --quiet "$DOTS_SERVICE"',
	enable = 'sudo systemctl enable "$DOTS_SERVICE"',
	disable = 'sudo systemctl disable "$DOTS_SERVICE"',
})
