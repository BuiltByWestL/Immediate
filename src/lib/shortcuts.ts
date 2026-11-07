const mac = window.__IMMEDIATE__?.platform === "macos";
export const MOD = mac ? "⌘" : "Ctrl";

export const SHORTCUTS: { group: string; items: [string[], string][] }[] = [
  {
    group: "Files",
    items: [
      [[MOD, "O"], "Open a file"],
      [[MOD, "V"], "Open what’s on the clipboard"],
      [[MOD, "N"], "New window"],
      [[MOD, "W"], "Close window"],
      [["←", "→"], "Previous / next file in the folder"],
      [["Home", "End"], "First / last file"],
      [[MOD, "C"], "Copy the picture"],
      [[MOD, "Shift", "C"], "Copy the file’s location"],
    ],
  },
  {
    group: "Viewing",
    items: [
      [["0"], "Fit to window"],
      [["1"], "Actual size"],
      [["+", "−"], "Zoom in / out"],
      [["Space"], "Play / pause"],
      [["I"], "Show or hide info"],
      [[mac ? "⌃⌘F" : "F11"], "Full screen"],
    ],
  },
  {
    group: "Editing photos",
    items: [
      [["E"], "Edit"],
      [[MOD, "S"], "Save a copy next to the original"],
      [[MOD, "Shift", "S"], "Save As"],
      [[MOD, "Z"], "Undo"],
      [[MOD, "Shift", "Z"], "Redo"],
      [["\\"], "Hold to compare with the original"],
      [["Esc"], "Cancel editing"],
    ],
  },
  {
    group: "App",
    items: [
      [[MOD, ","], "Settings"],
      [["?"], "Keyboard shortcuts"],
      [["Esc"], "Close a panel, leave full screen"],
    ],
  },
];
