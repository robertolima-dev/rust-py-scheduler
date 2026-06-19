import threading
import time

from rust_py_scheduler import Scheduler


def _run_in_background(scheduler):
    thread = threading.Thread(target=scheduler.run)
    thread.start()
    return thread


def test_max_retries_defaults_to_zero_so_one_failure_is_one_error():
    scheduler = Scheduler()
    attempts = []

    def always_fails():
        attempts.append(time.monotonic())
        raise RuntimeError("boom")

    scheduler.every("1s", always_fails)

    thread = _run_in_background(scheduler)
    time.sleep(1.4)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert len(attempts) == 1
    job = scheduler.list_jobs()[0]
    assert job["run_count"] == 0
    assert job["error_count"] == 1


def test_job_retries_on_failure_until_it_succeeds():
    scheduler = Scheduler()
    attempts = []

    def flaky():
        attempts.append(time.monotonic())
        if len(attempts) < 3:
            raise RuntimeError("not yet")

    job_id = scheduler.every("1s", flaky, max_retries=5)

    thread = _run_in_background(scheduler)
    time.sleep(1.4)
    scheduler.shutdown()
    thread.join(timeout=2)

    # 2 failed attempts + 1 successful one, all within the same tick.
    assert len(attempts) == 3
    job = scheduler.list_jobs()[0]
    assert job["id"] == job_id
    assert job["run_count"] == 1
    assert job["error_count"] == 0


def test_job_counts_as_error_only_after_exhausting_retries():
    scheduler = Scheduler()
    attempts = []

    def always_fails():
        attempts.append(time.monotonic())
        raise RuntimeError("boom")

    scheduler.every("1s", always_fails, max_retries=2)

    thread = _run_in_background(scheduler)
    time.sleep(1.4)
    scheduler.shutdown()
    thread.join(timeout=2)

    # initial attempt + 2 retries, all within the same tick.
    assert len(attempts) == 3
    job = scheduler.list_jobs()[0]
    assert job["run_count"] == 0
    assert job["error_count"] == 1


def test_max_retries_works_with_decorator_form():
    scheduler = Scheduler()
    attempts = []

    @scheduler.every("1s", max_retries=3)
    def flaky():
        attempts.append(time.monotonic())
        if len(attempts) < 2:
            raise RuntimeError("not yet")

    thread = _run_in_background(scheduler)
    time.sleep(1.4)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert len(attempts) == 2
    job = scheduler.list_jobs()[0]
    assert job["run_count"] == 1
    assert job["error_count"] == 0


def test_list_jobs_exposes_last_error_and_clears_it_on_a_later_success():
    scheduler = Scheduler()
    attempts = []

    def flaky():
        attempts.append(time.monotonic())
        if len(attempts) == 1:
            raise RuntimeError("first failure")

    scheduler.every("1s", flaky)

    thread = _run_in_background(scheduler)
    time.sleep(1.4)
    job_after_failure = scheduler.list_jobs()[0]
    assert job_after_failure["error_count"] == 1
    assert "first failure" in job_after_failure["last_error"]

    time.sleep(1.2)
    scheduler.shutdown()
    thread.join(timeout=2)

    job_after_success = scheduler.list_jobs()[0]
    assert job_after_success["run_count"] == 1
    assert job_after_success["last_error"] is None
