// CSV/TSV that round-trips: parsing remembers each cell's quoting so saving changes only what
// was edited.

export interface Table {
  rows: string[][];
  /** quoted[r][c]: the cell was in quotes in the file. */
  quoted: boolean[][];
  delimiter: string;
  /** The file ended with a line break. */
  trailingNewline: boolean;
}

export function detectDelimiter(text: string, ext: string): string {
  if (ext === "tsv") return "\t";
  const first = text.slice(0, 4000).split("\n").slice(0, 5).join("\n");
  const count = (d: string) => first.split(d).length - 1;
  return [",", ";", "\t", "|"].sort((a, b) => count(b) - count(a))[0];
}

export function parseCsv(text: string, delimiter: string): Table {
  const rows: string[][] = [];
  const quoted: boolean[][] = [];
  let row: string[] = [];
  let qrow: boolean[] = [];
  let cell = "";
  let inQuotes = false;
  let wasQuoted = false;
  let i = 0;
  const pushCell = () => {
    row.push(cell);
    qrow.push(wasQuoted);
    cell = "";
    wasQuoted = false;
  };
  const pushRow = () => {
    pushCell();
    rows.push(row);
    quoted.push(qrow);
    row = [];
    qrow = [];
  };
  while (i < text.length) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"') {
        if (text[i + 1] === '"') {
          cell += '"';
          i += 2;
          continue;
        }
        inQuotes = false;
      } else cell += ch;
    } else if (ch === '"' && cell === "") {
      inQuotes = true;
      wasQuoted = true;
    } else if (ch === delimiter) pushCell();
    else if (ch === "\n") pushRow();
    else cell += ch;
    i++;
  }
  const trailingNewline = text.endsWith("\n");
  if (!trailingNewline || cell !== "" || row.length) pushRow();
  return { rows, quoted, delimiter, trailingNewline };
}

export function serializeCsv(t: Table): string {
  const needs = (v: string) => v.includes(t.delimiter) || v.includes('"') || v.includes("\n") || v !== v.trim();
  const out = t.rows
    .map((r, ri) => r.map((v, ci) => (t.quoted[ri]?.[ci] || needs(v) ? `"${v.replace(/"/g, '""')}"` : v)).join(t.delimiter))
    .join("\n");
  return t.trailingNewline ? out + "\n" : out;
}
