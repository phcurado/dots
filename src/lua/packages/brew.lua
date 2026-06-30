dots.provider.package("brew", {
	available = "command -v brew >/dev/null",
	installed = 'brew list --formula "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'brew install "$DOTS_PACKAGE"',
	remove = 'brew uninstall "$DOTS_PACKAGE"',
})

dots.provider.package("brew-cask", {
	available = "command -v brew >/dev/null",
	installed = 'brew list --cask "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'brew install --cask "$DOTS_PACKAGE"',
	remove = 'brew uninstall --cask "$DOTS_PACKAGE"',
})

dots.brew.cask = dots["brew-cask"].install

dots.provider.package("brew-tap", {
	available = "command -v brew >/dev/null",
	installed = 'brew tap | grep -Fx "$DOTS_PACKAGE" >/dev/null 2>&1',
	install = 'brew tap "$DOTS_PACKAGE"',
	remove = 'brew untap "$DOTS_PACKAGE"',
})

dots.brew.tap = dots["brew-tap"].install

dots.brew.enable = function()
	dots.command("homebrew", {
		check = "command -v brew >/dev/null",
		apply = [[/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"]],
		provides = { "provider:brew", "provider:brew-cask", "provider:brew-tap", "provider:brew-service" },
	})
end
