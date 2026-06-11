"""Shift Board — weekly shift schedule. Single-file vibox demo app."""

import os
import sqlite3
from html import escape
from urllib.parse import parse_qs

from fastapi import FastAPI, Request
from fastapi.responses import HTMLResponse, RedirectResponse

DB_PATH = "/data/shifts.db"
DAYS = ("Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun")
SLOTS = ("morning", "afternoon", "evening")

os.makedirs("/data", exist_ok=True)
with sqlite3.connect(DB_PATH) as _conn:
    _conn.execute(
        """
        CREATE TABLE IF NOT EXISTS shifts (
            id INTEGER PRIMARY KEY,
            person TEXT,
            day TEXT,
            slot TEXT
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
.day-section { margin-bottom: 18px; }
.day-section:last-child { margin-bottom: 0; }
.day-name {
  font-size: 14px; font-weight: 700; color: #c17812; margin-bottom: 8px;
  padding-bottom: 6px; border-bottom: 1px solid #3a3a3a;
}
.shift-row {
  display: flex; align-items: center; justify-content: space-between;
  padding: 9px 4px; border-bottom: 1px solid #2e2e2e; font-size: 14px;
}
.shift-row:last-child { border-bottom: none; }
.slot { font-size: 12px; padding: 3px 12px; border-radius: 999px; border: 1px solid #3a3a3a; }
.slot-morning { color: #f0d9b8; }
.slot-afternoon { color: #c17812; }
.slot-evening { color: #e0b07f; }
.empty { color: #9a9a9a; font-size: 14px; padding: 6px 0; }
"""


def render_schedule(rows: list[sqlite3.Row]) -> str:
    if not rows:
        return '<p class="empty">No shifts scheduled yet — add the first one above.</p>'
    by_day: dict[str, list[sqlite3.Row]] = {}
    for r in rows:
        by_day.setdefault(r["day"], []).append(r)
    sections = []
    for day in DAYS:
        shifts = by_day.get(day)
        if not shifts:
            continue
        shifts.sort(key=lambda r: SLOTS.index(r["slot"]))
        shift_rows = "".join(
            f"""
            <div class="shift-row">
              <span>{escape(s["person"])}</span>
              <span class="slot slot-{escape(s["slot"])}">{escape(s["slot"])}</span>
            </div>"""
            for s in shifts
        )
        sections.append(
            f'<div class="day-section"><div class="day-name">{day}</div>{shift_rows}</div>'
        )
    return "".join(sections)


def render_page(rows: list[sqlite3.Row]) -> str:
    day_options = "".join(f'<option value="{d}">{d}</option>' for d in DAYS)
    slot_options = "".join(f'<option value="{s}">{s}</option>' for s in SLOTS)
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Shift Board</title>
  <style>{STYLE}</style>
</head>
<body>
  <div class="wrap">
    <header>
      <span class="logo">📅</span>
      <h1>Shift Board<span class="accent">.</span></h1>
    </header>
    <section class="card">
      <h2>Add shift</h2>
      <form method="post" action="/shifts">
        <div class="field">
          <label for="person">Person</label>
          <input id="person" name="person" required placeholder="Sam">
        </div>
        <div class="field">
          <label for="day">Day</label>
          <select id="day" name="day">{day_options}</select>
        </div>
        <div class="field">
          <label for="slot">Slot</label>
          <select id="slot" name="slot">{slot_options}</select>
        </div>
        <button type="submit">Add shift</button>
      </form>
    </section>
    <section class="card">
      <h2>This week</h2>
      {render_schedule(rows)}
    </section>
  </div>
</body>
</html>"""


@app.get("/", response_class=HTMLResponse)
def index() -> str:
    with db() as conn:
        rows = conn.execute("SELECT * FROM shifts ORDER BY id").fetchall()
    return render_page(rows)


@app.post("/shifts")
async def add_shift(request: Request) -> RedirectResponse:
    form = parse_qs((await request.body()).decode())
    person = form.get("person", [""])[0].strip()
    day = form.get("day", [""])[0]
    slot = form.get("slot", [""])[0]
    if person and day in DAYS and slot in SLOTS:
        with db() as conn:
            conn.execute(
                "INSERT INTO shifts (person, day, slot) VALUES (?, ?, ?)",
                (person, day, slot),
            )
    return RedirectResponse(url="/", status_code=303)
