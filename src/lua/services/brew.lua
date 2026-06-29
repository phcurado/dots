dots.provider.service("brew-service", {
	started = [[brew services list | awk -v service="$DOTS_SERVICE" '$1 == service && $2 == "started" { found = 1 } END { exit !found }]],
	start = 'brew services start "$DOTS_SERVICE"',
	stop = 'brew services stop "$DOTS_SERVICE"',
})

dots.brew.service = {
	start = dots["brew-service"].start,
}
