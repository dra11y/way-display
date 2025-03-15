# way-display

I am legally blind and created this crate because Wayland, instead of creating a new standard to replace the old standard, rendered `xrandr` obsolete and fractured the Linux community by making every single compositor and desktop environment implement display control in a different way, making life more complicated for all users, especially those with disabilities. For example, on Fedora 41 with GNOME on Wayland, GDM does not allow the user to select which screen their login prompt appears on, and Display Settings has been greatly curtailed and doesn't remember user preferences from one login to the next (it just defaults to a multi-monitor configuration, forcing the user to hit `Meta+P` a number of times, with only a visual feedback on the "main" display, which is not accessible).

This is but one of the many accessibility limitations introduced by Wayland (another major one being the lack of global keyboard input control, which is necessary for screen reader and screen magnification controls). It is unfathomable that, in 2025, with all the tools and resources available to Linux developers, accessibility on Linux is getting worse, not better.

New technology is supposed to improve the user experience, not curtail it. By forcing every compositor to re-invent the wheel, Wayland's imposition harms users, especially those with disabilities, and thus far, the Wayland authors have refused to prioritize accessibility. This not only harms users with disabilities, but all users, because it limits users' freedom to choose how their environment works, which was a core benefit of Linux. Unfortunately, everyone is jumping on the Wayland bandwagon, long before Wayland is "ready for prime time." Further, Wayland may never be "ready," because it is only a protocol, not a standard, and the fracturing will only get worse with time, not better, until the Wayland core authors introduce accessibility as a core component of their protocol.

Wayland's stated priorities were "security" and "graphics", not user experience or accessibility. This potentially harms all desktop vendors who adopt Wayland, because it opens both legal and social liabilities for those vendors (Red Hat and Canonical being two of the largest), especially with the entry into force of the European Accessibility Act of 2025.

While I agree that, "ignorance that is not willful is not malice," the converse follows. I hope that desktop vendors as well as users awaken to this major oversight of the Wayland authors and put pressure on the Wayland authors to awaken and start considering the needs of users with disabilities, as well as users in general who want to customize their experiences.

## What way-display attempts to do

Being legally blind, I have one large 42" external monitor (not two or three smaller monitors, on which the screen magnification experience is terrible in most operating systems).

When I plug my laptop into my Thunderbolt dock, I expect the display to switch, not "extend", to the external monitor. One would think this would be simple to achieve via user preference in Linux in 2025. Not on Wayland, it isn't. (On X11, I used scripting with `xrandr` to achieve this, but that's gone now, and the only workaround seems to be the quite technical and esoteric DBus protocol.)

Running `way-display external` from the command line forces the display to switch to the external monitor, and `way-display internal` the converse. Running `way-display both` switches both monitors on for mirroring.

`way-display status` shows the status of all attached displays, including the "logical" display, which is basically the "effective" display after applying the settings (`internal` or `external`).

`way-display -w external` watches the display...
