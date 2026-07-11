dots.provider.service("systemd", {
	capability = "provider:systemd",
	available = "command -v systemctl >/dev/null",
	started = 'systemctl is-active --quiet "$DOTS_SERVICE"',
	start = 'sudo systemctl start "$DOTS_SERVICE"',
	stop = 'sudo systemctl stop "$DOTS_SERVICE"',
	enabled = 'systemctl is-enabled --quiet "$DOTS_SERVICE"',
	enable = 'sudo systemctl enable "$DOTS_SERVICE"',
	disable = 'sudo systemctl disable "$DOTS_SERVICE"',
	list_started = "systemctl list-units --state=active --no-legend --no-pager | awk '{ print $1 }'",
	list_enabled = "systemctl list-unit-files --no-legend --no-pager | awk '$2 ~ /^(enabled|enabled-runtime|linked|linked-runtime)$/ { print $1 }'",
})
