Pixel Pet for Linux

Start it:      ./pixel-pet
Install it:    ./install.sh            (into your home folder, no root)
Remove it:     ./install.sh --remove
Close it:      right-click the pet, QUIT (or ./pixel-pet --toggle)

Needs an X server. On Wayland desktops it runs through XWayland, which nearly all of them have; the
pet then only sees windows that run through XWayland too, and the mouse only over those.

Right-click the pet (or Ctrl+Alt+P) for the hub. Settings are in ~/.config/pixel-pet.

Not there yet on Linux: dancing to music, what is playing, the tray icon, and OpenCode's tokens.
