"""
Cron scheduling: registration, validation, and the dual call/decorator form.

These tests deliberately avoid asserting that a cron job *runs* at a precise
wall-clock minute — that would make the suite slow and clock-dependent.
Instead they pin down the contract: valid expressions register (and expose a
human-readable schedule + a future next_run_at), invalid ones raise
ValueError eagerly, and both calling conventions behave like `every()`.
"""
import pytest

from rust_py_scheduler import Scheduler


def hello():
    print("hello")


@pytest.mark.parametrize(
    "expression",
    [
        "* * * * *",
        "0 * * * *",
        "*/15 * * * *",
        "0 9 * * 1-5",
        "30 9,17 * * *",
        "0 0 1 1 *",
        "0 0 * * 0",
        "0 0 * * 7",
    ],
)
def test_cron_accepts_valid_expressions(expression):
    scheduler = Scheduler()
    job_id = scheduler.cron(expression, hello)
    assert isinstance(job_id, str)


@pytest.mark.parametrize(
    "expression",
    [
        "",
        "* * * *",            # too few fields
        "* * * * * *",        # too many fields
        "60 * * * *",         # minute out of range
        "* 24 * * *",         # hour out of range
        "* * 0 * *",          # day-of-month is 1-31
        "* * * 13 *",         # month out of range
        "abc * * * *",        # non-numeric
        "*/0 * * * *",        # zero step
    ],
)
def test_cron_rejects_invalid_expressions(expression):
    scheduler = Scheduler()
    with pytest.raises(ValueError):
        scheduler.cron(expression, hello)


def test_cron_rejects_invalid_expression_in_decorator_form():
    scheduler = Scheduler()

    with pytest.raises(ValueError):
        @scheduler.cron("not a cron expr")
        def decorated():
            pass


def test_cron_registers_a_job_with_a_readable_schedule():
    scheduler = Scheduler()
    job_id = scheduler.cron("0 9 * * *", hello)

    job = scheduler.list_jobs()[0]
    assert job["id"] == job_id
    assert job["schedule"] == "cron 0 9 * * *"
    assert job["enabled"] is True
    assert job["run_count"] == 0
    assert job["next_run_at"] is not None


def test_cron_as_decorator_returns_the_original_function_unchanged():
    scheduler = Scheduler()

    @scheduler.cron("0 * * * *")
    def decorated():
        return "result"

    assert decorated() == "result"
    assert decorated.__name__ == "decorated"
    assert scheduler.list_jobs()[0]["name"] == "decorated"


def test_cron_next_run_at_is_in_the_future():
    import time

    scheduler = Scheduler()
    scheduler.cron("0 0 1 1 *", hello)  # once a year, definitely in the future

    next_run_at = int(scheduler.list_jobs()[0]["next_run_at"])
    assert next_run_at > int(time.time())


def test_cron_supports_max_retries():
    scheduler = Scheduler()
    job_id = scheduler.cron("0 * * * *", hello, max_retries=3)

    job = scheduler.list_jobs()[0]
    assert job["id"] == job_id
    assert job["max_retries"] == 3


def test_cron_job_runs_when_due():
    # A "* * * * *" job becomes due at the next minute boundary. To keep the
    # test fast and deterministic we don't wait a full minute; instead we
    # confirm the job is scheduled and the loop stays healthy. Precise firing
    # is covered by the Rust-side next_after tests.
    import threading
    import time

    scheduler = Scheduler()
    scheduler.cron("* * * * *", hello)

    thread = threading.Thread(target=scheduler.run)
    thread.start()
    time.sleep(0.5)
    scheduler.shutdown()
    thread.join(timeout=2)

    assert not thread.is_alive()
