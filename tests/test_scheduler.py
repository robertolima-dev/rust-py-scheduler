import threading
import time

from rust_py_scheduler import Scheduler


def test_scheduler_can_be_instantiated():
    scheduler = Scheduler()
    assert scheduler is not None


def test_scheduler_repr_uses_correct_module():
    scheduler = Scheduler()
    assert type(scheduler).__module__ == "rust_py_scheduler"
    assert type(scheduler).__name__ == "Scheduler"


def _run_in_background(scheduler):
    thread = threading.Thread(target=scheduler.run)
    thread.start()
    return thread


def test_run_executes_due_jobs_until_shutdown():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    thread = _run_in_background(scheduler)
    time.sleep(2.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert not thread.is_alive()
    assert len(calls) >= 2


def test_run_keeps_going_after_a_job_raises(capsys):
    scheduler = Scheduler()
    calls = []

    def boom():
        calls.append("boom")
        raise RuntimeError("falha proposital")

    def healthy():
        calls.append("healthy")

    scheduler.every("1s", boom)
    scheduler.every("1s", healthy)

    thread = _run_in_background(scheduler)
    time.sleep(2.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert not thread.is_alive()
    assert calls.count("boom") >= 2
    assert calls.count("healthy") >= 2

    captured = capsys.readouterr()
    assert "RuntimeError: falha proposital" in captured.err
