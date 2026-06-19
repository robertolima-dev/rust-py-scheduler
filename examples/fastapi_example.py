"""
FastAPI integration: the scheduler starts with the app and stops when it does.

Run with:
    pip install "rust-py-scheduler[fastapi]"
    uvicorn examples.fastapi_example:app --reload
"""
from fastapi import FastAPI

from rust_py_scheduler import Scheduler
from rust_py_scheduler.fastapi import scheduler_lifespan

scheduler = Scheduler()


@scheduler.every("10s")
def heartbeat():
    print("heartbeat (every 10s)")


@scheduler.cron("0 * * * *")
def hourly_report():
    print("hourly report (top of every hour)")


# Starts the scheduler on app startup, shuts it down on app shutdown.
app = FastAPI(lifespan=scheduler_lifespan(scheduler))


@app.get("/jobs")
def jobs():
    return scheduler.list_jobs()
