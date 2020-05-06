# auto-invite-matrix-bot

Bot allows redirect people, that send messages to one of your "wrong" (old, abandoned, not active) Matrix accounts to your real account.

It does join on invite, invites your "real" account and sends a customizable message, support listening multiple accounts or Homeservers.

Also it can relay mentions from your secondary accounts to your primary account.

## Usage
1. Install rust on your system
2. Make a config file according to the config_example.yml
3. Install the application using `cargo install --git https://github.com/MTRNord/auto-invite-matrix-bot.git`
4. Run it with `auto-invite-matrix-bot` with an addition `--config` argument to point to your config file
