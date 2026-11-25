# Morrow mascot options

Four original mascot directions for the idle notch. None of these derive from the CodeIsland artwork. They are SVG on purpose: the idle mascot renders at **20 pt** in the collapsed pill, so a vector asset stays crisp at every scale and can be dropped straight into `Assets.xcassets` (Xcode 15+ supports SVG) as `Image("MascotPip")` and friends.

Each file is a self-contained 64×64 vector with a dark `#1C1C1E` backing so you can judge contrast against the actual notch pill. The backing is part of the preview, not the asset — drop the whole SVG in for a quick look, then remove the `<rect>` when exporting the final transparent asset.

| Option | Preview | Concept | Why it could work | Risk at 20 pt |
|---|---|---|---|---|
| **Pip** (rejected) | [preview](previews/pip.png) | Dawn songbird, gold → coral | "Morrow" means morning; a bird is the most literal, warm reading. Simple teardrop silhouette and two dot eyes survive tiny sizes. | Rejected on 2026-11-25: the bird direction was dropped after the 20 pt review in favour of Nori. |
| **Nori** (chosen) | [preview](previews/nori.png) | Cat that lives in the notch | Ear tufts echo the notch's two concave top ears, so it reads as native to the surface. | Dark navy body on a dark pill is the weakest contrast of the four; the amber eyes carry it at 20 pt. |
| **Sprout** | [preview](previews/sprout.png) | Morning leaf with a face | Fresh, calm, not affiliated with any AI brand; green stands out on charcoal. | Face circle sits low; the leaves carry the silhouette, not the expression. |
| **Ollo** | [preview](previews/ollo.png) | Small owl, lavender | Watchful companion fits an app that watches agent sessions; highest "friendly but not childish" score. | Two face disks are busy at 20 pt; eyes may blur into one blob. |

Vectors: [`pip.svg`](pip.svg) · [`nori.svg`](nori.svg) · [`sprout.svg`](sprout.svg) · [`ollo.svg`](ollo.svg)

## Recommendation

**Nori** is the mascot for #44. Its ear tufts echo the notch's concave ears, the amber eyes stay legible at 20 pt on the dark pill, and it is not tied to an AI brand. Pip was the first pick but was rejected on 2026-11-25 after the 20 pt comparison. **Ollo** remains the runner-up if a watcher mood is wanted later.

## Next step to use one

1. Pick an option; remove the background `<rect>` from the SVG.
2. For Nori, the shipped path is transparent `Canvas` paths in `MorrowIdleMascotView.swift`; an `Assets.xcassets` vector image is the alternative.
3. Keep the slow blink/bob on `MascotTimeline` so the idle mark stays alive without a new timer.
4. Verify at 20 pt on a real non-notch Display and a hardware-notch Mac before merging.
