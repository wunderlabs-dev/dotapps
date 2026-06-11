"""Cafe Tracker v2 — adds weekly totals. Swapped in live during the demo (cp v2-app.py app.py)."""

import os
import sqlite3
from html import escape
from urllib.parse import parse_qs

from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse, RedirectResponse

DB_PATH = "/data/cafe.db"
ROASTS = ("dark", "medium", "light")

os.makedirs("/data", exist_ok=True)
with sqlite3.connect(DB_PATH) as _conn:
    _conn.execute(
        """
        CREATE TABLE IF NOT EXISTS deliveries (
            id INTEGER PRIMARY KEY,
            supplier TEXT,
            kg REAL,
            roast TEXT,
            created_at TEXT DEFAULT current_timestamp
        )
        """
    )

app = FastAPI()


def db() -> sqlite3.Connection:
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn


STYLE = """
* { box-sizing: border-box; margin: 0; padding: 0; }
body {
  background: #191919; color: #ffffff; min-height: 100vh;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
}
.wrap { max-width: 860px; margin: 0 auto; padding: 40px 24px 64px; }
header { display: flex; align-items: baseline; gap: 14px; margin-bottom: 28px; }
header .logo { font-size: 34px; }
header h1 { font-size: 26px; font-weight: 700; letter-spacing: 0.2px; }
header .accent { color: #c17812; }
.tag {
  font-size: 11px; font-weight: 700; color: #c17812; border: 1px solid #c17812;
  border-radius: 999px; padding: 2px 9px; letter-spacing: 0.6px; align-self: center;
}
.card {
  background: #252525; border: 1px solid #333333; border-radius: 12px;
  padding: 22px 24px; margin-bottom: 24px;
}
.card h2 {
  font-size: 13px; font-weight: 600; text-transform: uppercase;
  letter-spacing: 1.4px; color: #c17812; margin-bottom: 16px;
}
form { display: flex; gap: 12px; flex-wrap: wrap; align-items: flex-end; }
.field { display: flex; flex-direction: column; gap: 6px; }
.field label { font-size: 12px; color: #9a9a9a; letter-spacing: 0.4px; }
input, select {
  background: #1f1f1f; border: 1px solid #3a3a3a; border-radius: 8px;
  color: #ffffff; padding: 10px 12px; font-size: 14px; min-width: 150px;
  outline: none; appearance: none;
}
input:focus, select:focus { border-color: #c17812; }
button {
  background: #c17812; color: #191919; border: none; border-radius: 8px;
  padding: 11px 22px; font-size: 14px; font-weight: 700; cursor: pointer;
}
button:hover { background: #d8891a; }
table { width: 100%; border-collapse: collapse; }
th {
  text-align: left; font-size: 11px; text-transform: uppercase; letter-spacing: 1.2px;
  color: #c17812; padding: 8px 10px; border-bottom: 1px solid #3a3a3a;
}
td { padding: 11px 10px; font-size: 14px; border-bottom: 1px solid #2e2e2e; }
tr:last-child td { border-bottom: none; }
td.num { text-align: right; font-variant-numeric: tabular-nums; }
th.num { text-align: right; }
.roast { font-size: 12px; padding: 3px 10px; border-radius: 999px; border: 1px solid #3a3a3a; }
.roast-dark { color: #e0b07f; }
.roast-medium { color: #c17812; }
.roast-light { color: #f0d9b8; }
.muted { color: #9a9a9a; }
.empty { color: #9a9a9a; font-size: 14px; padding: 6px 0; }
.totals { display: flex; gap: 12px; flex-wrap: wrap; }
.total-chip {
  background: #1f1f1f; border: 1px solid #3a3a3a; border-radius: 10px;
  padding: 12px 18px; display: flex; flex-direction: column; gap: 4px; min-width: 130px;
}
.total-chip .roast-name {
  font-size: 11px; text-transform: uppercase; letter-spacing: 1.2px; color: #9a9a9a;
}
.total-chip .roast-kg { font-size: 22px; font-weight: 700; color: #c17812; }
"""


def render_totals(totals: list[sqlite3.Row]) -> str:
    if not totals:
        return '<p class="empty">No deliveries yet this week.</p>'
    chips = "".join(
        f"""
        <div class="total-chip">
          <span class="roast-name">{escape(t["roast"])}</span>
          <span class="roast-kg">{t["total_kg"]:g} kg</span>
        </div>"""
        for t in totals
    )
    return f'<div class="totals">{chips}</div>'


def render_page(rows: list[sqlite3.Row], totals: list[sqlite3.Row]) -> str:
    if rows:
        body_rows = "".join(
            f"""
            <tr>
              <td>{escape(r["supplier"])}</td>
              <td class="num">{r["kg"]:g}</td>
              <td><span class="roast roast-{escape(r["roast"])}">{escape(r["roast"])}</span></td>
              <td class="muted">{escape(r["created_at"])}</td>
            </tr>"""
            for r in rows
        )
        table = f"""
        <table>
          <thead>
            <tr><th>Supplier</th><th class="num">Kg</th><th>Roast</th><th>Logged (UTC)</th></tr>
          </thead>
          <tbody>{body_rows}</tbody>
        </table>"""
    else:
        table = '<p class="empty">No deliveries logged yet — add the first one above.</p>'

    roast_options = "".join(f'<option value="{r}">{r}</option>' for r in ROASTS)
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Cafe Tracker</title>
  <style>{STYLE}</style>
</head>
<body>
  <div class="wrap">
    <header>
      <span class="logo">☕</span>
      <h1>Cafe Tracker<span class="accent">.</span></h1>
      <span class="tag">v2</span>
    </header>
    <section class="card">
      <h2>New delivery</h2>
      <form method="post" action="/deliveries">
        <div class="field">
          <label for="supplier">Supplier</label>
          <input id="supplier" name="supplier" required placeholder="Acme Beans">
        </div>
        <div class="field">
          <label for="kg">Kilograms</label>
          <input id="kg" name="kg" type="number" step="0.1" min="0" required placeholder="12.5">
        </div>
        <div class="field">
          <label for="roast">Roast</label>
          <select id="roast" name="roast">{roast_options}</select>
        </div>
        <button type="submit">Log delivery</button>
      </form>
    </section>
    <section class="card">
      <h2>This week</h2>
      {render_totals(totals)}
    </section>
    <section class="card">
      <h2>Deliveries</h2>
      {table}
    </section>
  </div>
</body>
</html>"""


@app.get("/", response_class=HTMLResponse)
def index() -> str:
    with db() as conn:
        rows = conn.execute("SELECT * FROM deliveries ORDER BY id DESC").fetchall()
        totals = conn.execute(
            """
            SELECT roast, SUM(kg) AS total_kg FROM deliveries
            WHERE created_at > date('now', '-7 day')
            GROUP BY roast ORDER BY total_kg DESC
            """
        ).fetchall()
    return render_page(rows, totals)


@app.post("/deliveries")
async def add_delivery(request: Request) -> RedirectResponse:
    form = parse_qs((await request.body()).decode())
    supplier = form.get("supplier", [""])[0].strip()
    roast = form.get("roast", [""])[0]
    try:
        kg = float(form.get("kg", [""])[0])
    except ValueError:
        kg = 0.0
    if supplier and kg > 0 and roast in ROASTS:
        with db() as conn:
            conn.execute(
                "INSERT INTO deliveries (supplier, kg, roast) VALUES (?, ?, ?)",
                (supplier, kg, roast),
            )
    return RedirectResponse(url="/", status_code=303)
