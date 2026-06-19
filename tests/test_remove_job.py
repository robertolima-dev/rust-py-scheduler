import threading
import time

import pytest

from rust_py_scheduler import Scheduler


def test_remove_job_removes_a_registered_job():
    scheduler = Scheduler()
    job_id = scheduler.every("5m", lambda: None)

    scheduler.remove_job(job_id)

    assert scheduler.list_jobs() == []


def test_remove_job_raises_key_error_for_unknown_id():
    scheduler = Scheduler()

    with pytest.raises(KeyError):
        scheduler.remove_job("not-a-real-id")


def test_remove_job_raises_key_error_if_called_twice():
    scheduler = Scheduler()
    job_id = scheduler.every("5m", lambda: None)
    scheduler.remove_job(job_id)

    with pytest.raises(KeyError):
        scheduler.remove_job(job_id)


def test_remove_job_stops_future_executions():
    scheduler = Scheduler()
    calls = []
    job_id = scheduler.every("1s", lambda: calls.append(time.monotonic()))

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(1.5)

    scheduler.remove_job(job_id)
    calls_at_removal = len(calls)
    assert calls_at_removal >= 1

    time.sleep(1.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert len(calls) == calls_at_removal


def test_remove_job_does_not_affect_other_jobs():
    scheduler = Scheduler()
    job_id_to_remove = scheduler.every("5m", lambda: None)
    job_id_to_keep = scheduler.every("10m", lambda: None)

    scheduler.remove_job(job_id_to_remove)

    remaining_ids = [job["id"] for job in scheduler.list_jobs()]
    assert remaining_ids == [job_id_to_keep]
