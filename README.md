# ttyrec

The ttyrec file format is used to record terminal sessions for later
playback. This is commonly used for recording terminal games such as
[NetHack](https://alt.org/nethack) or [Dungeon Crawl Stone
Soup](https://crawl.akrasiac.org/), but can be used for any terminal session.

## Recording

Sessions can be recorded using the `ttyrec` command. It launches a shell, and
all output from that session is saved to a file, similar to the [`script`
command from
util-linux](https://www.man7.org/linux/man-pages/man1/script.1.html). Unlike
`script`, however, `ttyrec` also saves information about the timing between
chunks of output, so that the session can be played back in real time as it
happened. See `ttyrec --help` for more information about available options.

## Playback

The `ttyplay` command is an interactive player for ttyrec files. In addition to
playing back files as they were recorded, this player allows for arbitrary
seeking forwards and backwards through the file, pausing, adjusting the
playback speed, and searching for output content. See `ttyplay --help` for more
information about available options, and press `?` while the player is paused
(via the Space key) to see a list of key bindings.
