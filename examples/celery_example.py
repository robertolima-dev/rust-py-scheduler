"""
Using rust_py_scheduler to *trigger* Celery tasks on a schedule.

There is no `rust_py_scheduler.celery` module, and on purpose: a Celery task's
`.delay` / `.apply_async` are just callables, so the existing `every()` /
`cron()` API already schedules them with nothing new to learn. This file is
the whole "integration" — a pattern, not a dependency.

The scheduler runs in *your* process and only enqueues messages; the Celery
worker (a separate process) does the actual work. That keeps the scheduler
tiny and lets Celery own retries, routing, and concurrency.

Run the worker in one terminal:
    celery -A examples.celery_example worker --loglevel=info

Run this scheduler in another:
    python examples/celery_example.py
"""
import time

from celery import Celery

from rust_py_scheduler import Scheduler

app = Celery(
    "rust_py_scheduler_demo",
    broker="redis://localhost:6379/0",
    backend="redis://localhost:6379/1",
)


@app.task
def send_report(name: str) -> str:
    # Runs inside the Celery worker, not in the scheduler process.
    print(f"[worker] generating report for {name}")
    return f"report:{name}"


def main() -> None:
    scheduler = Scheduler()

    # `.delay(...)` returns immediately (it just publishes to the broker), so
    # it's a perfect scheduler callback — the scheduler never blocks on work.
    scheduler.every("5m", lambda: send_report.delay("daily-metrics"))

    # Cron works the same way — enqueue a task every weekday at 8am.
    scheduler.cron("0 8 * * 1-5", lambda: send_report.delay("weekday-digest"))

    # Need countdown/eta/queue routing? Use apply_async in the lambda:
    scheduler.every(
        "1h",
        lambda: send_report.apply_async(args=["hourly"], countdown=10),
    )

    scheduler.start_background()
    print("Scheduler running; Ctrl+C to stop.")
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        scheduler.shutdown()
        print("\nScheduler shut down.")


if __name__ == "__main__":
    main()
