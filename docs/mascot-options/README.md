# Morrow mascot options

The idle mascot renders at **20 pt** in the collapsed pill, so it is drawn as vector `Canvas` paths and stays crisp at every scale. Preview renders were reviewed in chat during the design pass but are intentionally not stored in this repository.

## Chosen: Lumen

Lumen is one character with two renderings, selected by the existing `islandUITheme` AppStorage key:

| Theme | Drawing | Finish |
|---|---|---|
| Modern | `MorrowIdleMascotView.drawModern` | Smooth indigo → blue gradient body, glass highlight, rounded glowing cyan eyes, antenna. |
| Pixel | `MorrowIdleMascotView.drawPixel` | Hard-stepped 16-bit blocks, limited blue/cyan palette, square eyes, pixel antenna. |

Same silhouette and same eyes in two finishes; flipping the setting switches both instantly. Neither drawing includes the dark preview backdrop. Both keep the slow blink and bob on `MascotTimeline`, and `IdleMascotSource.resolved` still decides when the mascot appears.

## Rejected options

- **Pip**, dawn songbird — rejected by the user.
- **Nori**, notch cat — rejected by the user.
- **Sprout**, morning leaf — not chosen; the leaf silhouette reads weakly at 20 pt.
- **Ollo**, little owl — not chosen; two face disks are busy at 20 pt.

This file is the text record of that decision, not a set of image assets.
