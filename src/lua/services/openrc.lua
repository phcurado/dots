dots.provider.service("openrc", {
	capability = "provider:openrc",
	available = "command -v rc-service >/dev/null && command -v rc-update >/dev/null",
	started = 'rc-service "$DOTS_SERVICE" status >/dev/null 2>&1',
	start = 'sudo rc-service "$DOTS_SERVICE" start',
	stop = 'sudo rc-service "$DOTS_SERVICE" stop',
	enabled = 'rc-update show default | awk \'{ print $1 }\' | grep -qx "$DOTS_SERVICE"',
	enable = 'sudo rc-update add "$DOTS_SERVICE" default',
	disable = 'sudo rc-update del "$DOTS_SERVICE" default',
	list_enabled = "rc-update show default | awk '{ print $1 }'",
})
