dots.provider.package("dnf", {
	capability = "provider:dnf",
	available = "command -v dnf >/dev/null && command -v rpm >/dev/null",
	installed = 'rpm -q "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'sudo dnf install -y "$DOTS_PACKAGE"',
	remove = 'sudo dnf remove -y "$DOTS_PACKAGE"',
	list = "rpm -qa --qf '%{NAME}\\n'",
})
