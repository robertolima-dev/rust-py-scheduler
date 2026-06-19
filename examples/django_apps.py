"""
Django integration: start the scheduler from your app's AppConfig.ready().

This is what an app's `apps.py` looks like. Point your app's `default` config
at `MyAppConfig` (e.g. `default_app_config` or `INSTALLED_APPS`) and the
scheduler starts when Django boots the app.

    pip install "rust-py-scheduler[django]"

Heads-up for gunicorn/uwsgi with workers > 1: each worker process runs
ready() and therefore its own scheduler, so jobs run once per worker. For
exactly-once cluster-wide scheduling, run the scheduler in a single dedicated
process instead (e.g. a management command that calls scheduler.run()).
"""
from django.apps import AppConfig

from rust_py_scheduler import Scheduler
from rust_py_scheduler.django import start_in_background

scheduler = Scheduler()


@scheduler.every("5m")
def refresh_cache():
    print("refreshing cache (every 5m)")


@scheduler.cron("30 2 * * *")
def nightly_cleanup():
    print("nightly cleanup (02:30 every day)")


class MyAppConfig(AppConfig):
    name = "myapp"

    def ready(self):
        # Idempotent: safe even if ready() is called more than once. Also
        # registers a best-effort atexit shutdown for normal process exit.
        start_in_background(scheduler)
