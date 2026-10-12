---
name: Oaclis
description: Monochrome fullscreen device workspace with contextual chat transfers.
colors:
  bg: "#fafafa"
  panel: "#fff"
  ink: "#171717"
  muted: "#666"
  line: "#dedede"
  soft: "#efefef"
  dot: "#e4e4e4"
  dark-bg: "#111"
  dark-panel: "#1b1b1b"
  dark-ink: "#f5f5f5"
  dark-muted: "#aaa"
  dark-line: "#383838"
  dark-soft: "#282828"
  dark-dot: "#252525"
typography:
  body:
    fontFamily: 'ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif'
    fontSize: "14px"
  brand:
    fontSize: "18px"
    fontWeight: 600
    letterSpacing: "-.035em"
  title:
    fontSize: "15px"
    fontWeight: 550
    lineHeight: 1.6
  node:
    fontSize: "13px"
    fontWeight: 550
    lineHeight: 1.5
  label:
    fontSize: "12px"
rounded:
  control: "5px"
  toolbar: "9px"
  surface: "10px"
spacing:
  compact: "8px"
  field: "10px"
  node: "20px"
components:
  button-primary:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.panel}"
    rounded: "{rounded.control}"
    padding: "11px"
    width: "100%"
  button-icon:
    textColor: "{colors.muted}"
    rounded: "{rounded.control}"
    padding: "9px"
  field:
    backgroundColor: "{colors.bg}"
    textColor: "{colors.ink}"
    rounded: "{rounded.control}"
    padding: "{spacing.field}"
  map-node:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.ink}"
    rounded: "{rounded.surface}"
    padding: "{spacing.node}"
    width: "204px"
---

# Design System: Oaclis

## Overview

A quiet, minimal, modern and orderly device workspace. The confirmed visual references are Tenzen Studio and ElevenLabs; SPAAIC informs the local object-map interaction. The entire viewport belongs to devices, with contextual content revealed by selection. White, black and grays are the only palette.

This record describes `ui/style.css`, `ui/index.html` and `ui/app.js`. It replaces the previous three-column shell. Independent finish review is pending; this documentation does not assert a ship verdict. There are no shipping raster assets; screenshots are review artifacts only.

## Colors

### Primary

Ink is the action color: primary buttons, keyboard focus, reachable-device indicators and progress. Panel provides its inverse text. Selection and drop feedback combine ink borders with soft fill.

**The Monochrome State Rule.** Use labels, shape and neutral contrast to explain state; do not add colored success or error accents.

### Neutral

Background anchors the canvas; panel holds devices and floating controls. Muted distinguishes secondary text, line defines outlines, soft marks hover and selection, and dot draws the functional canvas grid. The frontmatter preserves the canonical CSS values. Dark entries map directly to the seven semantic overrides in `:root[data-theme="dark"]`; they are not additional accents.

## Typography

Use the platform sans stack without font downloads. Brand is the only persistent wordmark; there is no static page headline. Node names, contextual chat titles and field labels use the modest hierarchy recorded above. Chat rows use label-sized text at weight 500 and line-height 1.65. Paths and long names wrap anywhere. Transfer metrics use tabular numerals.

## Layout

The canvas is fixed to all viewport edges, with a functional dotted grid repeated every 24px. The body does not scroll. Minimal floating toolbars sit at the upper and lower left, 22px from the vertical edge and 24px from the left. The upper toolbar contains Oaclis, add-device and check; the lower contains reorder, sound and theme.

The contextual drawer sits at upper right, 350px wide and bounded to `calc(100dvh - 100px)`. Its chat list and detail content scroll internally. Activity occupies the lower right only during a transfer or while its result remains visible. There are no permanent library, details or activity columns.

At the implemented maximum-width 640px breakpoint, toolbars use 14px offsets. Nodes narrow to 160px, with 16px padding and smaller icons. The drawer spans between 14px side margins, starts at 70px and is bounded to `calc(100dvh - 150px)`. Activity spans the same margins, 70px above the bottom. Notices occupy the lower right. The canvas remains fullscreen; mobile does not restore a stacked page shell. Default nodes form a centered grid with 32px gaps and columns that fit the viewport. Additional rows extend the vertically scrollable canvas; saved positions also contribute to its height. Node movement is clamped to the canvas bounds and persisted locally; reorder clears saved positions.

## Elevation & Depth

Flat tonal layering and single-pixel borders distinguish canvas, nodes, toolbars and drawer. There are no shadows or decorative gradients. Layering follows toolbars, drawer, activity, then notices. Buttons and fields transition background color over 150ms with ease-out; reduced-motion preference removes these transitions.

## Shapes

Compact controls use the control radius, toolbars and activity the toolbar radius, and nodes and drawer the surface radius. Small round status marks distinguish reachable from unverified/offline devices. Inline stroked SVG icons use rounded caps and joins, without an icon runtime. Device icons are larger than action icons.

## Components

### Buttons

Icon actions have accessible names and neutral hover/pressed feedback. Primary actions span their form and use ink fill with panel text; hover reduces opacity to .85. Disabled buttons reduce opacity to .45 and use the wait cursor. Every focus-visible element receives a two-pixel ink outline with four-pixel offset.

### Inputs / Fields

Native inputs and selects use background fill, a line border, control radius and compact padding. Labels remain explicit; search has its own accessible name. Paths are editable only inside the initially collapsed native details disclosure. Native validation remains active.

### Contextual Drawer

Selecting a source device opens its actual Codex chat titles with search and refresh. Local and remote catalog reads are read-only; remote access uses native SSH/Python. Selecting a chat replaces the list with its title, source route, destination, copy/sync operation and explicit start action. Back returns to chats. Chat and project creation stay in Codex; paths are associated details, not standalone library entries. Device add/edit uses the same contextual surface.

Opening a device focuses search after loading; selecting a chat or opening a device form focuses its heading. Refresh preserves an existing focused chat row. Close and Escape dismiss the drawer and restore focus to the selected source node when it exists. The drawer remains a nonmodal aside without a focus trap.

### Device Canvas

Device nodes are keyboard-focusable buttons with connection status in their accessible names. Enter/Space opens chats; arrow keys move nodes by 10px. Pointer dragging moves only layout. Dragging a chat onto another device prepares a proposal, never executes a transfer. The destination form provides the alternative to dragging. Remote-to-remote transfer is not offered; one endpoint is this device.

### Transfer Activity

Hide activity while idle. Show actual preparation, transfer, verification, completion, conflict or failure state; progress is indeterminate when totals are unknown. Completed results have a dismiss action. Brief notices use a polite live region; sound is opt-in. Do not imply that all activity updates are live-announced.

## Do's and Don'ts

- Do reuse the seven semantic variables and their dark overrides.
- Do keep the initial viewport to devices and minimal floating commands.
- Do preserve visible focus, native disclosures and reduced-motion support.
- Do expose actual device chat titles and actual transfer state.
- Don't restore permanent columns, page titles or explanatory blocks.
- Don't invent topology lines, connectivity or progress.
- Don't introduce decorative gradients, remote fonts, raster decoration or CDN dependencies.
- Don't conflate node movement, transfer proposal and execution.

Library rows: Codex title followed by the full chat UUID in small muted text. No row icon, type label or file size. Search accepts title or ID.

Le chat mostrano titolo e UUID, con un badge circolare contenente l’iniziale quando non è scelta un’icona. La matita modifica label locale, icona e colore senza modificare Codex. I clienti hanno due porte laterali: bidirezionale continua, monodirezionale tratteggiata. Il collegamento della chat selezionata usa il suo colore e badge ed è visibile quando il cliente è raggiungibile; rappresenta la proposta selezionata, non una regola automatica salvata.

Le icone possono essere caricate e sostituite (PNG/JPEG/WebP/GIF, fino a 5 MB), normalizzate in PNG 64×64 e salvate nel database locale appearance.sqlite. Il colore della chat tinge lo sfondo di library-row; la matita è un pulsante piccolo posizionato in alto a destra senza colonna dedicata. Nessun attributo style viene generato nell’HTML; gli stili dinamici si applicano via DOM.

Ogni dispositivo, incluso il master, ha un colore personalizzabile dalla matita sul nodo, salvato come device:<id> nello stesso database delle chat. Il colore tinge nodo, bordo, icona e porte; le linee continuano a usare il colore della chat. Lo sfondo riprende il campo di punti dello stile classico di struttura.html (passo 20 px), nei due temi.

La proposta di sincronizzazione ha tre icone selezionabili (chat, cartelle, app/plugin/configurazioni), inizialmente attive, senza testo permanente e con nomi accessibili/tooltip. Le selezioni persistono nel database per chat e sorgente. Un loading compatto rappresenta l’analisi effettiva, blocca Avvia fino al risultato e scompare quando pronta; errore e riprova sono contestuali. Nessun campo manuale per i percorsi. L’analisi effimera, il database e la preparazione dell’ambiente stanno fuori dalla logica della UI.
