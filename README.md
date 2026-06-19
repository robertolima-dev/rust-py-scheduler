# rust-py-scheduler

Lightweight, performant task scheduler for Python applications, with a Rust core.

Register interval-based jobs (`"10s"`, `"5m"`, `"1h"`) or cron jobs (`"0 9 * * 1-5"`) with a plain function call or a decorator, run them in-process or on a background thread, and inspect their state (run/error counts, last error, next run time) at any point — without a database or an external broker. First-class FastAPI, Django, and Celery integrations are included.

---

## Features

- **`Scheduler`** — simple API: `every()`, `cron()`, `list_jobs()`, `remove_job()`, `run()`, `start_background()`, `shutdown()`
- **Two ways to register a job** — a direct call (`scheduler.every("5s", fn)`) or a decorator (`@scheduler.every("5s")`); both return the same registration
- **Interval scheduling** — `"10s"`, `"5m"`, `"1h"` (seconds/minutes/hours)
- **Cron scheduling** — standard 5-field Unix expressions, e.g. `scheduler.cron("0 9 * * 1-5", fn)` for weekdays at 9am
- **Framework integrations** — start/stop with the app lifecycle on **FastAPI** and **Django**, and trigger **Celery** tasks on a schedule
- **Run in-process or in the background** — block the calling thread with `run()`, or call `start_background()` to schedule on a dedicated OS thread and keep going
- **Per-job retries** — `max_retries=N` retries a failing job up to `N` extra times, immediately, before counting it as an error
- **Jobs never crash the loop** — an exception in one job is caught, printed, and tracked (`error_count`, `last_error`); every other job keeps running on schedule
- **Rust core** — scheduling, timing, retries, and thread management run in Rust via PyO3; the Python API stays small and easy to read

---

## Requirements

- Python 3.10+
- No required runtime dependencies (the extension is a self-contained native module, built for the stable ABI — one wheel works across 3.10–3.13+)

---

## Installation

```bash
pip install rust-py-scheduler
```

With framework integrations:

```bash
pip install "rust-py-scheduler[fastapi]"   # FastAPI + uvicorn
pip install "rust-py-scheduler[django]"    # Django 4.2+
pip install "rust-py-scheduler[celery]"    # Celery
```

For running the test suite:

```bash
pip install "rust-py-scheduler[tests]"
```

---

## Quick Start

```python
import time

from rust_py_scheduler import Scheduler

scheduler = Scheduler()

# Direct call: registers immediately and returns the job id.
job_id = scheduler.every("2s", lambda: print("tick (direct call)"))


# Decorator form: same registration, but `report` stays a normal,
# directly-callable function afterwards.
@scheduler.every("3s", max_retries=2)
def report():
    print("tick (decorator, with retry budget)")


scheduler.start_background()  # returns immediately, runs on its own OS thread
time.sleep(7)

for job in scheduler.list_jobs():
    print(job)

scheduler.remove_job(job_id)
scheduler.shutdown()
```

See [`examples/basic_usage.py`](examples/basic_usage.py) for the full, runnable version of this script.

---

## Registering Jobs

```python
scheduler.every("5s", my_function)                 # direct call
scheduler.every("5s", my_function, max_retries=3)   # with a retry budget

@scheduler.every("5s")                              # decorator
def my_function():
    ...

@scheduler.every("5s", max_retries=3)               # decorator, with retries
def my_function():
    ...
```

- `interval` accepts an integer amount followed by `s` (seconds), `m` (minutes), or `h` (hours) — e.g. `"30s"`, `"5m"`, `"2h"`. Anything else (empty, missing unit, zero, negative, non-numeric) raises `ValueError` immediately, when `every()` is called — not later, when the job would have run.
- The decorator form always returns the original function unchanged (`__name__`, behavior, everything) — it's safe to keep calling it directly elsewhere in your code.
- `max_retries` (default `0`) works identically in both forms — see [Error Handling & Retries](#error-handling--retries).

---

## Cron Scheduling

For calendar-based schedules ("every weekday at 9am", "top of every hour"), use `cron()` with a standard 5-field Unix expression. It has the exact same dual call/decorator API as `every()`, including `max_retries`.

```python
scheduler.cron("0 * * * *", my_function)          # every hour, on the hour
scheduler.cron("*/15 * * * *", my_function)        # every 15 minutes

@scheduler.cron("0 9 * * 1-5")                     # weekdays at 9am
def morning_report():
    ...

@scheduler.cron("30 2 * * *", max_retries=2)       # daily at 02:30, with retries
def nightly_cleanup():
    ...
```

The five fields are `minute hour day-of-month month day-of-week`:

| Field | Range | Notes |
|---|---|---|
| minute | `0–59` | |
| hour | `0–23` | |
| day of month | `1–31` | |
| month | `1–12` | |
| day of week | `0–7` | `0` and `7` both mean Sunday |

Each field supports `*`, a single number (`5`), a range (`9-17`), a step (`*/15`, `9-17/2`), and comma-separated lists of those (`0,30`, `9-11,17`). When **both** day-of-month and day-of-week are restricted (neither is `*`), a time matches if **either** field matches — the same rule as Vixie cron.

- **Timezone:** expressions are evaluated in the **system's local timezone**.
- **Resolution:** cron has minute resolution; the smallest meaningful interval is one minute. For sub-minute schedules, use `every("30s", ...)`.
- An invalid expression (wrong field count, out-of-range value, bad syntax) raises `ValueError` immediately at registration, just like `every()`.

---

## Framework Integrations

### FastAPI

Start the scheduler with the app and stop it on shutdown, via the modern `lifespan` API:

```python
from fastapi import FastAPI
from rust_py_scheduler import Scheduler
from rust_py_scheduler.fastapi import scheduler_lifespan

scheduler = Scheduler()

@scheduler.every("30s")
def heartbeat():
    ...

app = FastAPI(lifespan=scheduler_lifespan(scheduler))
```

Already have a lifespan? Compose with it: `scheduler_lifespan(scheduler, your_lifespan)` — your startup runs after the scheduler is up, your shutdown before it stops. See [`examples/fastapi_example.py`](examples/fastapi_example.py).

### Django

Start the scheduler from your app's `AppConfig.ready()`:

```python
# apps.py
from django.apps import AppConfig
from rust_py_scheduler import Scheduler
from rust_py_scheduler.django import start_in_background

scheduler = Scheduler()

@scheduler.every("5m")
def refresh_cache():
    ...

class MyAppConfig(AppConfig):
    name = "myapp"

    def ready(self):
        start_in_background(scheduler)
```

`start_in_background()` is idempotent per process (calling `ready()` twice won't start a second thread) and registers a best-effort `atexit` shutdown for normal process exit.

> **Multi-worker note:** under gunicorn/uwsgi with `workers > 1`, every worker process runs `ready()` and therefore its own scheduler — jobs run once *per worker*, with no cross-process coordination. For exactly-once cluster-wide scheduling, run the scheduler in a single dedicated process (e.g. a management command calling `scheduler.run()`). See [`examples/django_apps.py`](examples/django_apps.py).

### Celery

There's no `rust_py_scheduler.celery` module — and that's deliberate. A Celery task's `.delay` / `.apply_async` is just a callable, so the existing API schedules it with nothing new to learn:

```python
@app.task
def send_report(name):
    ...

scheduler.every("5m", lambda: send_report.delay("daily-metrics"))
scheduler.cron("0 8 * * 1-5", lambda: send_report.delay("weekday-digest"))

# countdown/eta/routing? use apply_async:
scheduler.every("1h", lambda: send_report.apply_async(args=["hourly"], countdown=10))
```

The scheduler runs in your process and only *enqueues* messages; the Celery worker (a separate process) does the work — so Celery keeps owning retries, routing, and concurrency. See [`examples/celery_example.py`](examples/celery_example.py).

---

## Background Execution

```python
scheduler.run()  # blocks the calling thread until shutdown() is called
```

```python
scheduler.start_background()  # returns immediately; scheduling continues on a new OS thread
...
scheduler.shutdown()  # stops the loop and waits for the background thread to finish
```

- `run()` releases the GIL while idle, so other Python threads (e.g. one calling `shutdown()`) keep running normally.
- `start_background()` raises `RuntimeError` if called again while already running.
- `shutdown()` is safe to call even if the scheduler was never started, and safe to call from inside a job callback. It is **one-way**: once stopped, a `Scheduler` can't be resumed — start a new one instead. This matches typical usage (start once at application startup, shut down once at teardown).

---

## Inspecting and Removing Jobs

```python
for job in scheduler.list_jobs():
    print(job)
# {'id': '...', 'name': 'report', 'schedule': 'every 3s', 'enabled': True,
#  'run_count': 4, 'error_count': 0, 'last_run_at': '1718721000',
#  'next_run_at': '1718721003', 'max_retries': 2, 'last_error': None}

scheduler.remove_job(job_id)  # raises KeyError if job_id doesn't exist
```

`last_run_at`/`next_run_at` are Unix timestamps (seconds since the epoch) as strings, or `None` if the job hasn't run yet.

---

## Error Handling & Retries

A job that raises an exception never stops the scheduler or any other job — the traceback is printed to stderr, and the job's own `error_count` is incremented.

```python
@scheduler.every("10s", max_retries=2)
def flaky():
    ...
```

`max_retries=2` means up to 3 total attempts (the initial one + 2 retries) happen back-to-back, with no delay, before that scheduling tick is counted as a failure. `run_count`/`error_count` reflect *ticks*, not individual attempts: if any attempt within a tick succeeds, the whole tick counts as a success and `last_error` is cleared. Every failed attempt is still printed to stderr, even if a later attempt in the same tick succeeds.

| Situation | Exception |
|---|---|
| Invalid `interval` passed to `every()`, or invalid cron `expression` passed to `cron()` | `ValueError` |
| `start_background()` called while already running | `RuntimeError` |
| `remove_job()` called with an unknown id | `KeyError` |
| Exception raised inside a job callback | Caught internally — never raised to your code |

---

## API Reference

### `Scheduler()`

Creates a new, empty scheduler.

### `scheduler.every(interval, callback=None, max_retries=0)`

Registers an interval job. Called with `callback`, registers immediately and returns the job id (`str`); called without it (as `@scheduler.every(interval)`), returns a decorator that registers the function it's applied to and hands it back unchanged. Raises `ValueError` on an invalid `interval`.

### `scheduler.cron(expression, callback=None, max_retries=0)`

Registers a cron job from a 5-field Unix expression (`minute hour day-of-month month day-of-week`), evaluated in local time. Same dual call/decorator API and return values as `every()`. Raises `ValueError` on an invalid expression. See [Cron Scheduling](#cron-scheduling).

### `scheduler.list_jobs() -> list[dict]`

Snapshot of every registered job.

| Key | Type | Description |
|---|---|---|
| `id` | `str` | UUID v4 |
| `name` | `str` | The callback's `__name__` (or `"job"` if it has none) |
| `schedule` | `str` | Human-readable, e.g. `"every 300s"` or `"cron 0 9 * * 1-5"` |
| `enabled` | `bool` | Always `True` for now (toggling is planned) |
| `run_count` | `int` | Successful ticks |
| `error_count` | `int` | Failed ticks (after exhausting retries) |
| `last_run_at` | `str \| None` | Unix timestamp of the last execution |
| `next_run_at` | `str \| None` | Unix timestamp of the next scheduled execution |
| `max_retries` | `int` | Configured retry budget |
| `last_error` | `str \| None` | Message from the most recent failed attempt; cleared on success |

### `scheduler.remove_job(job_id)`

Unregisters a job. Raises `KeyError` if `job_id` doesn't exist.

### `scheduler.run()`

Blocks the calling thread, executing due jobs until `shutdown()` is called.

### `scheduler.start_background()`

Runs the same loop as `run()` on a dedicated OS thread and returns immediately. Raises `RuntimeError` if already running.

### `scheduler.shutdown()`

Stops the loop (background or not) and waits for the background thread to finish, if any. Safe to call multiple times, or when nothing was ever started.

---

## Building from Source

Requires Rust and [maturin](https://github.com/PyO3/maturin).

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install maturin

# Development build (installs into the current Python environment)
maturin develop

# Release wheel
maturin build --release
```

### Running tests

```bash
# Rust unit tests
PYO3_PYTHON="$(pwd)/.venv/bin/python3" cargo test --no-default-features --lib

# Python integration tests
pip install -e ".[tests]"
pytest
```

---

## Architecture

```
Python API (rust_py_scheduler)
    ├── Scheduler(...)               ──► src/scheduler.rs (PyO3 #[pyclass])
    │       ├── every()               ──► src/job.rs       (Job, Schedule model)
    │       │                         ──► src/interval.rs  (parses "10s"/"5m"/"1h")
    │       ├── cron()                ──► src/cron.rs       (parses "0 9 * * 1-5", next run)
    │       ├── list_jobs()           ──► src/registry.rs  (JobRegistry snapshot)
    │       ├── remove_job()          ──► src/registry.rs  (JobRegistry.remove)
    │       ├── run()                 ──► src/executor.rs  (run_loop, StopSignal)
    │       ├── start_background()    ──► run_loop() spawned on its own OS thread
    │       └── shutdown()            ──► StopSignal.stop() + thread join
    ├── rust_py_scheduler.fastapi    ──► scheduler_lifespan() (pure Python)
    └── rust_py_scheduler.django     ──► start_in_background() (pure Python)

src/registry.rs    ──► thread-safe job storage (Arc<Mutex<HashMap<...>>>); calls each
                       callback under the GIL, applies the retry loop, tracks counts
src/cron.rs        ──► 5-field cron parser; computes the next wall-clock occurrence and
                       converts the gap into a monotonic Instant deadline
src/time_utils.rs  ──► wall-clock timestamps for last_run_at/next_run_at (display only;
                       scheduling itself uses a monotonic std::time::Instant)
src/errors.rs      ──► SchedulerError -> PyErr (ValueError / RuntimeError / KeyError)
```

The core is compiled into a native extension (`.so`/`.pyd`) by [maturin](https://github.com/PyO3/maturin) and [PyO3](https://pyo3.rs), built against Python's stable ABI (`abi3-py310`) so a single wheel covers Python 3.10 through 3.13+. The Python layer (`python/rust_py_scheduler/__init__.py`) just re-exports the compiled module.

---

## Roadmap

Done: cron expressions, FastAPI / Django / Celery integrations, and a CI suite that runs the Rust + Python tests on every push. Still planned:

- Per-job `enabled` toggling (pause/resume without removing)
- Configurable cron timezone (today: system local time)
- Publish to TestPyPI, then PyPI

---

## License

MIT.
