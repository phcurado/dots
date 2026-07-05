dots.provider.package("zypper", {
	capability = "provider:zypper",
	available = "command -v zypper >/dev/null && command -v rpm >/dev/null",
	installed = 'rpm -q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'sudo zypper --non-interactive install "$DOTS_PACKAGE"',
	remove = 'sudo zypper --non-interactive remove "$DOTS_PACKAGE"',
	list = "rpm -qa --qf '%{NAME}\\n'",
})
