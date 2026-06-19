"""
Basic rust_py_scheduler usage, no web framework involved.

Run with:
    python examples/basic_usage.py
"""
import time

from rust_py_scheduler import Scheduler


def main() -> None:
    scheduler = Scheduler()

    # Direct call: registers immediately and returns the job id.
    job_id = scheduler.every("2s", lambda: print("tick (direct call)"))
    print("Registered job:", job_id)

    # Decorator form: same registration, but `report` stays a normal,
    # directly-callable function afterwards.
    @scheduler.every("3s", max_retries=2)
    def report():
        print("tick (decorator, with retry budget)")

    # Cron form: same dual call/decorator API, but a 5-field Unix expression
    # (minute hour day-of-month month day-of-week), evaluated in local time.
    @scheduler.cron("0 9 * * 1-5")
    def weekday_morning():
        print("tick (weekdays at 9am)")

    print("\nJobs currently registered:")
    for job in scheduler.list_jobs():
        print(job)

    # start_background() returns immediately; the scheduler keeps running
    # on its own OS thread until shutdown() is called.
    scheduler.start_background()

    print("\nRunning in the background for 7 seconds...")
    time.sleep(7)

    print("\nJobs after running for a while:")
    for job in scheduler.list_jobs():
        print(job)

    scheduler.remove_job(job_id)
    print(f"\nRemoved job {job_id}; only 'report' keeps running now.")

    time.sleep(3)

    scheduler.shutdown()
    print("\nScheduler shut down.")


if __name__ == "__main__":
    main()
