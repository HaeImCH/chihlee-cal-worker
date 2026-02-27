# chihlee-cal-to-csv

A Rust library + CLI for extracting table-like data from text PDFs into CSV.

## Features

- Extract table rows from text-based PDFs (no OCR).
- Auto-detect table-like line groups.
- Manual fallback areas via `--area page:x1,y1,x2,y2`.
- Merge all detected tables into one CSV.
- Include metadata columns: `page`, `table_id`.

## CLI

Build:

```bash
cargo build --release
```

Run:

```bash
./target/release/pdf2csv extract -i input.pdf -o output.csv
```

Options:

- `--pages 1-3,5`: Page selection.
- `--area page:x1,y1,x2,y2`: Manual table area (repeatable).
- `--delimiter ,`: CSV delimiter.
- `--has-header`: Treat first row as header.
- `--no-header`: Keep first row as data.
- `--min-cols 2`: Minimum columns per candidate row.
- `--clean-calendar`: Keep only calendar date/event style rows.
- `--nopage` (or `-nopage`): Remove the `page` column from output.
- `--notable` (or `-notable`): Remove the `table_id` column from output.
- `--custom-col-name date,event` (or `-custom_col_name date,event`): Rename `col_1,col_2`.
- `-v, --verbose`: Print detailed warnings.

Exit codes:

- `0`: Export succeeded and at least one row was written.
- `2`: Completed but no table rows were found.
- `1`: Error.

## Library API

```rust
use chihlee_cal_to_csv::{extract_pdf_to_csv, ExtractOptions};

let options = ExtractOptions::default();
let report = extract_pdf_to_csv("input.pdf".as_ref(), "output.csv".as_ref(), &options)?;
println!("rows={}, tables={}", report.row_count, report.table_count);
```

## Notes and Limitations

- Intended for text PDFs; scanned/image PDFs are out of scope.
- Table detection is heuristic and best-effort.
- Manual area mode currently falls back at page granularity because `pdf-extract` does not expose geometry primitives directly.
