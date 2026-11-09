const mac = window.__IMMEDIATE__?.platform === "macos";
export const MOD = mac ? "⌘" : "Ctrl";

export const SHORTCUTS: { group: string; items: [string[], string][] }[] = [
  {
    group: "Files",
    items: [
      [[MOD, "O"], "Open a file"],
      [[MOD, "V"], "Open what’s on the clipboard"],
      [[MOD, "N"], "New window"],
      [[MOD, "R"], "Record audio"],
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
    group: "Editing video and audio",
    items: [
      [["Space", "K"], "Play / pause"],
      [["J", "L"], "Back / forward 5 seconds"],
      [["←", "→"], "Previous / next frame (Shift: 1 second)"],
      [["Q"], "Trim the start to the playhead"],
      [["E"], "Trim the end to the playhead"],
      [["W"], "Cut out the selected part (or Delete); with nothing selected, split at the playhead"],
      [[MOD, "S"], "Save a copy next to the original"],
      [[MOD, "Z"], "Undo"],
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
