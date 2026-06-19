import threading

import pytest

from rust_py_scheduler import Scheduler


def hello():
    print("hello")


def test_every_registers_a_job_and_returns_an_id():
    scheduler = Scheduler()
    job_id = scheduler.every("1s", hello)
    assert isinstance(job_id, str)
    assert len(job_id) > 0


def test_every_returns_a_different_id_per_job():
    scheduler = Scheduler()
    job_id_1 = scheduler.every("1s", hello)
    job_id_2 = scheduler.every("5m", hello)
    assert job_id_1 != job_id_2


def test_every_as_decorator_returns_the_original_function_unchanged():
    scheduler = Scheduler()

    @scheduler.every("1s")
    def decorated():
        return "result"

    assert decorated() == "result"
    assert decorated.__name__ == "decorated"


def test_every_as_decorator_actually_registers_the_job():
    import time

    scheduler = Scheduler()
    calls = []

    @scheduler.every("1s")
    def decorated():
        calls.append("ran")

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(1.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert calls


def test_every_as_decorator_validates_interval_eagerly():
    scheduler = Scheduler()

    with pytest.raises(ValueError):
        @scheduler.every("not-a-valid-interval")
        def decorated():
            pass
