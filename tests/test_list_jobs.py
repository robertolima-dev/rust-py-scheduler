import threading
import time

from rust_py_scheduler import Scheduler


def test_list_jobs_is_empty_for_a_fresh_scheduler():
    scheduler = Scheduler()
    assert scheduler.list_jobs() == []


def test_list_jobs_reports_freshly_registered_jobs():
    scheduler = Scheduler()
    job_id = scheduler.every("5m", lambda: None)

    jobs = scheduler.list_jobs()
    assert len(jobs) == 1

    job = jobs[0]
    assert job["id"] == job_id
    assert job["schedule"] == "every 300s"
    assert job["enabled"] is True
    assert job["run_count"] == 0
    assert job["error_count"] == 0
    assert job["last_run_at"] is None
    assert job["next_run_at"] is not None


def test_list_jobs_uses_the_decorated_function_name():
    scheduler = Scheduler()

    @scheduler.every("1s")
    def decorated():
        pass

    assert scheduler.list_jobs()[0]["name"] == "decorated"


def test_list_jobs_reflects_run_count_and_last_run_at_after_executing():
    scheduler = Scheduler()
    scheduler.every("1s", lambda: None)

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(1.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    job = scheduler.list_jobs()[0]
    assert job["run_count"] >= 1
    assert job["last_run_at"] is not None


def test_list_jobs_counts_errors_separately_from_successful_runs():
    scheduler = Scheduler()

    def boom():
        raise RuntimeError("falha proposital")

    scheduler.every("1s", boom)

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(1.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    job = scheduler.list_jobs()[0]
    assert job["error_count"] >= 1
    assert job["run_count"] == 0
