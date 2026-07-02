dots.provider.service("brew-service", {
	capability = "provider:brew",
	available = "command -v brew >/dev/null",
	started = [[brew services list | awk -v service="$DOTS_SERVICE" '$1 == service && $2 == "started" { found = 1 } END { exit !found }]],
	start = 'brew services start "$DOTS_SERVICE"',
	stop = 'brew services stop "$DOTS_SERVICE"',
	list_started = "brew services list | awk 'NR > 1 && $2 == \"started\" { print $1 }'",
})

dots.brew.service = {
	start = dots["brew-service"].start,
}
