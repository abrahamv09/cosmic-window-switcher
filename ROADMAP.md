# COSMIC Window Switcher Roadmap

## Version 1

Deliver fast, predictable Window switching through MRU Order and Live Thumbnails. The Switcher Grid selects and activates Windows and supports one deliberate management action: moving a Window to another workspace.

- Drag a Switcher Item to move its Window to another workspace, following the COSMIC workspace-overview interaction.

This remains a v1 release requirement, but implementation is gated on resolving or verifying COSMIC's currently inconsistent workspace-move capability advertisement. The switcher must not issue an unadvertised request or ship a non-functional drop target.

## Version 2

- Close a Window from its Switcher Item.
- Minimize or restore a Window from its Switcher Item.
- Open a workspace picker for the selected Switcher Item through right-click or keyboard.
