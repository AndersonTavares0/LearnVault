# LearnVault brand guidelines

The identity says one thing: a private academy kept in a vault. These are the rules for using it. The design record is [`DESIGN.md`](../../../DESIGN.md) (see its Brand section); this page is the short version for anyone adding a screen, a document or an asset.

## The mark

The **Academy Portico** is a Greek school front, with a pediment, four columns and steps, and an arched door that is the vault. The stone is one neutral colour. The door and the dot in the pediment are the only accent, gold, so the mark never competes with the learner's own accent colour.

## Files

All brand assets are vector (`.svg`) and live here. The hygiene check refuses raster images in this folder.

| File | Use |
| --- | --- |
| `learnvault-mark.svg` | The mark alone, on light surfaces |
| `learnvault-mark-dark.svg` | The mark alone, on dark surfaces |
| `learnvault-icon.svg` | The full mark on a warm-ink tile: the app icon, and the source of `crates/desktop/icons/icon.png` and the 48 px and larger sizes of `icon.ico` |
| `../../../apps/web-next/public/favicon.svg` | The reduced cut on the tile: the favicon, and the source of the 16 px and 32 px sizes of `icon.ico` |
| `learnvault-seal.svg`, `learnvault-seal-dark.svg` | The mark inside a ring that reads LEARNVAULT · SAPERE AUDE, for large placements: the README, certificates, an About page |
| `learnvault-banner.svg`, `learnvault-banner-light.svg` | The README header uses the light banner on every theme. The dark banner is kept for dark surfaces such as slides or social previews |

Text in every file is converted to outlines, so nothing depends on a font being installed. The lettering is Playfair Display, bold for the name and the ring, medium italic for the motto.

`crates/desktop/icons/icon.png` and `icon.ico` are the only raster files allowed in the repository for the brand. They are generated from the SVGs above, never drawn by hand.

## Colour

| Colour | Value | Use |
| --- | --- | --- |
| Warm ink | `#1C1815` | The stone on light surfaces, text on the gold, the icon tile |
| Cream | `#F5EEE2` | The stone on dark surfaces, light backgrounds |
| Gold on dark | `#E0A83E` | The door and the dot on dark surfaces. 8.3:1 on warm ink |
| Gold on light | `#A8761F` | The door and the dot on light surfaces. 3.5:1 on cream |

The gold on light surfaces is for graphics. At 3.5:1 it passes as a graphic (3:1) but is too faint for small text, so text is ink. The one exception is the light README banner, where the tagline is muted ink `#5E5143` (6.7:1 on cream) and the motto is dark brass `#795514` (5.8:1, the `reward` text token in `DESIGN.md`). Both pass 4.5:1. The bright gold is never used for text.

In the app, the stone follows the text colour (`text-foreground`) and the door follows the reward colour, which is these same golds. The default accent is the Ink preset. See `DESIGN.md` for the tokens.

## Size and space

- **Full mark:** 32 px and larger. Below that use the reduced cut (two columns, a wider door): the `compact` prop of `BrandMark` for sizes of 24 px and below, and `favicon.svg` for 16 px and 32 px.
- **Seal:** 72 px and larger. The ring text is not legible below that, so use the mark instead.
- **Clear space:** keep at least one eighth of the mark's width free on every side.
- **With the name:** the mark sits to the left of "LearnVault" in Playfair Display bold. In the top bar the mark is 32 px, the name is 24 px, and the gap is 10 px.

## Do not

- Recolour the stone gold, or the door anything but gold.
- Put the gold door on a gold or brown background, or the dark-surface colours on a light one.
- Stretch, rotate, outline, add shadows or gradients.
- Redraw the mark, or the seal's lettering, by hand. Change the generator input and regenerate.
- Use the logo to suggest endorsement by a school or an accrediting body. LearnVault is a personal study tool, and its certificate page says so.

## Mottos

*Sapere aude* ("dare to know", Horace, *Epistles*) is the motto of the project. It sits beside the mark on the sign-in page and never inside it, apart from the seal's ring. A few other lines from antiquity appear in context, one per screen:

| Where | Line | English | Portuguese |
| --- | --- | --- | --- |
| Sign in | *Sapere aude* | Dare to know | Ouse saber |
| Progress | *Nulla dies sine linea* | Not a day without a line | Nenhum dia sem uma linha |
| Import a course | *Ad fontes* | Back to the sources | De volta às fontes |
| Empty library | *ἀρχὴ ἥμισυ παντός* | The beginning is half of the whole | O começo é metade do todo |

Rules:

- Always show the translation in the learner's language beside the quotation, through `tr(en, ptBr)`, and mark the quotation with its `lang` attribute (`la` or `grc`). The `Motto` component does both.
- Set Greek in Literata. Playfair Display has no Greek glyphs.
- One motto per screen. Never a rotating quote on every page.
- Do not use the inscription "Let no one untrained in geometry enter" over the Academy's door: the source calls it unverifiable, and it says the opposite of an app open to any learner.
- The Portuguese lines are the maintainers' own translations. A new line needs a native reader before it ships.
- A new line needs a verified source, and the pull request should name it.

## Changing the identity

A change to the mark, the colours or the mottos is a pull request like any other, with these extras:

1. Regenerate the PNG and ICO from the SVGs and check the 16 px favicon by eye.
2. Update the Brand section of `DESIGN.md` and the app's `BrandMark` and `Motto` components together.
3. Check contrast: any colour added must reach 4.5:1 as text and 3:1 as a graphic on all eight surface palettes, light and dark.
4. Provide the Portuguese text for every new string.
5. Run the repository hygiene check: it refuses raster images and large files outside the allowed folders.
