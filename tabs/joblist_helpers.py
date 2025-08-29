# tabs/joblist_helpers.py

import os
from datetime import datetime, timedelta

# ---------- Column detection ----------

def _norm(s: str) -> str:
    return str(s or "").strip().lower().replace(" ", "").replace("_", "")

def _find_columns(tree):
    """
    Detect the important columns by fuzzy name match.
    Returns a dict with keys:
      name, interval, interval_value, interval_unit, last_save, last_size, next_save
    Any missing keys will be None.
    """
    # Try different ways to get columns
    cols = []
    try:
        cols = list(tree["columns"])
    except:
        try:
            cols = list(getattr(tree, "columns", ()))
        except:
            try:
                cols = list(tree.cget("columns"))
            except:
                pass
    
    # If we still can't get columns, use the known column names
    if not cols:
        cols = ['name', 'interval', 'NextSave', 'LastSave', 'LastSize']
    
    # Direct mapping since we know the exact column names
    result = {
        "name":         "name" if "name" in cols else None,
        "interval":     "interval" if "interval" in cols else None,
        "interval_value": "interval_value" if "interval_value" in cols else None,
        "interval_unit":  "interval_unit" if "interval_unit" in cols else None,
        "last_save":    "LastSave" if "LastSave" in cols else None,
        "last_size":    "LastSize" if "LastSize" in cols else None,
        "next_save":    "NextSave" if "NextSave" in cols else None,
    }
    
    return result

# ---------- Formatting ----------

def _fmt_size(n: int | None) -> str:
    if not n:
        return "—"
    size = float(n)
    for unit in ("B", "KB", "MB", "GB", "TB", "PB"):
        if size < 1024 or unit == "PB":
            return f"{size:.0f} {unit}" if unit == "B" else f"{size:.2f} {unit}"
        size /= 1024.0
    return f"{size:.2f} PB"

def _fmt_dt(dt: datetime | None) -> str:
    return dt.strftime("%Y-%m-%d %H:%M") if isinstance(dt, datetime) else "—"

def _fmt_interval(value: int | str | None, unit: str | None) -> str:
    if value is None or value == "":
        return "—"
    try:
        v = int(value)
    except Exception:
        return str(value)
    u = (unit or "").lower()
    if u.startswith("sec"):  suf = "sec"
    elif u.startswith("min"): suf = "min"
    elif u.startswith("hour"): suf = "hour" if v == 1 else "hours"
    elif u.startswith("day"):  suf = "day" if v == 1 else "days"
    else: suf = unit or ""
    if suf in ("min", "sec"):
        return f"{v} {suf}"
    return f"{v} {suf}".strip()

# ---------- Data providers ----------

def _interval_seconds(manager, job: dict) -> int:
    # uses the patched method if present, else derives from job keys
    if hasattr(manager, "_interval_seconds"):
        return int(manager._interval_seconds(job))
    value = int(job.get("interval_value", 0) or 0)
    unit  = str(job.get("interval_unit", "minutes")).lower()
    if unit.startswith("sec"):  return max(1, value)
    if unit.startswith("min"):  return max(60, value * 60)
    if unit.startswith("hour"): return max(3600, value * 3600)
    if unit.startswith("day"):  return max(86400, value * 86400)
    return max(60, value * 60)

def _latest(manager, job):
    if hasattr(manager, "get_latest_backup"):
        return manager.get_latest_backup(job)
    # graceful fallback: no info
    return None, None, None

def _next_run(manager, job, last_dt: datetime | None) -> datetime | None:
    # prefer patched _next_run_at if present
    nxt = job.get("_next_run_at")
    if isinstance(nxt, datetime):
        return nxt
    if not isinstance(last_dt, datetime):
        return None
    return last_dt + timedelta(seconds=_interval_seconds(manager, job))

# ---------- Main row renderer ----------

def render_last_save_and_size(tree, item_id, job: dict, manager):
    """
    Sets columns for LastSave, LastSize, NextSave, Interval (or IntervalValue/Unit),
    depending on which columns your Treeview actually has.
    """
    cols = _find_columns(tree)

    # Fetch latest backup details
    path, last_dt, size_bytes = _latest(manager, job)

    # Compute/format strings
    last_str  = _fmt_dt(last_dt)
    size_str  = _fmt_size(size_bytes)
    next_dt   = _next_run(manager, job, last_dt)
    next_str  = _fmt_dt(next_dt)

    ival = job.get("interval_value", job.get("interval", ""))
    iunit = job.get("interval_unit", "")
    interval_str = _fmt_interval(ival, iunit)

    # Interval: set either a combined "Interval" column, or separate value/unit columns
    if cols["interval"]:
        try: 
            tree.set(item_id, cols["interval"], interval_str)
        except Exception: 
            pass
    else:
        if cols["interval_value"]:
            try: tree.set(item_id, cols["interval_value"], str(ival))
            except Exception: pass
        if cols["interval_unit"]:
            try: tree.set(item_id, cols["interval_unit"], str(iunit))
            except Exception: pass

    # Last Save
    if cols["last_save"]:
        try: 
            tree.set(item_id, cols["last_save"], last_str)
        except Exception: 
            pass

    # Last Size
    if cols["last_size"]:
        try: 
            tree.set(item_id, cols["last_size"], size_str)
        except Exception: 
            pass

    # Next Save
    if cols["next_save"]:
        try: 
            tree.set(item_id, cols["next_save"], next_str)
        except Exception: 
            pass

    # Optional: mark overdue
    try:
        overdue = True if last_dt is None else ((datetime.now() - last_dt).total_seconds() >= _interval_seconds(manager, job))
        if overdue:
            tree.item(item_id, tags=("overdue",))
            tree.tag_configure("overdue", foreground="#ff5252")
    except Exception:
        pass
