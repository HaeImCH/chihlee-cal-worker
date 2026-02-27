# API Reference

This worker exposes calendar link APIs under `/api/v1`.

## Base URL

- Local: `http://127.0.0.1:8787`
- Production: your deployed Worker URL

## Common Error Response

All endpoints return this JSON shape on error:

```json
{
  "code": "bad_request",
  "message": "semester must be within 0..=999"
}
```

Error codes and status mapping:

- `unauthorized` -> `401`
- `bad_request` -> `400`
- `not_found` -> `404`
- `upstream_error` -> `502`
- `parse_error` -> `422`
- `validation_error` -> `422`
- `internal_error` -> `500`

---

## 1) GET `/api/v1/current_semester`

Returns the current ROC-year semester summary.

### Response 200

```json
{
  "semester": 114,
  "roc_year": 115,
  "target": 114,
  "latest_available": 114,
  "source_url": "https://www.chihlee.edu.tw/p/404-1000-62149.php",
  "cached": true
}
```

Notes:

- `target` is `roc_year - 1`.
- `semester` is `target` if available in source links; otherwise `-1`.

---

## 2) GET `/api/v1/cal_link`

Returns semester PDF link data.

### Query Params

- `semester` (optional, integer `0..=999`)
- `force` (optional, truthy if `true`, `1`, or `yes`) to bypass cache and rebuild CSV immediately
- `all` (optional, truthy if `true`, `1`, or `yes`, case-insensitive)

### Response 200 (single; default)

`GET /api/v1/cal_link` or `GET /api/v1/cal_link?semester=114`

```json
{
  "semester": 114,
  "url": "https://www.chihlee.edu.tw/.../114.pdf",
  "resolved_by": "current",
  "cached": true
}
```

`resolved_by`:

- `explicit`: `semester` query param provided
- `current`: no `semester`, and `roc_year - 1` exists
- `latest`: no `semester`, fallback to newest available

### Response 200 (all links)

`GET /api/v1/cal_link?all=true`

```json
{
  "items": [
    {
      "semester": 114,
      "url": "https://www.chihlee.edu.tw/.../114.pdf",
      "title": "Academic Calendar 114"
    },
    {
      "semester": 113,
      "url": "https://www.chihlee.edu.tw/.../113.pdf",
      "title": "Academic Calendar 113"
    }
  ],
  "cached": true
}
```

---

## 3) GET `/api/v1/csv`

Returns extracted CSV for a semester.

### Query Params

- `semester` (optional, integer `0..=999`)

If `semester` is omitted, selection follows the same behavior as `/api/v1/cal_link`:

- prefer current (`roc_year - 1`)
- fallback to latest available

### Response 200

- Content-Type: `text/csv; charset=utf-8`
- `X-Cache-Status`: `HIT` | `MISS` | `BYPASS`
- Header columns are fixed to: `date,event`
- `page` and `table_id` columns are not included

Example:

```csv
date,event
9/2-3,全校導師知能研習
9/9,轉學生入學輔導
```

Extraction mode is aligned with:

- `--clean-calendar`
- `--nopage`
- `--notable`
- `--custom_col_name date,event`

---

## Environment / Bindings

### Optional

- `SOURCE_URL`

## Cache and Cron

- CSV cache TTL: 120 days (`10,368,000` seconds)
- Scheduled job: `0 2 * * *` (UTC), refreshes all discovered semester PDFs and re-extracts CSV
