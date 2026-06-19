import time

import pytest

from rust_py_scheduler import Scheduler


def test_start_background_does_not_block_the_caller():
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    started_at = time.monotonic()
    scheduler.start_background()
    elapsed_to_return = time.monotonic() - started_at

    try:
        assert elapsed_to_return < 0.5
        time.sleep(2.5)
        assert len(calls) >= 2
    finally:
        scheduler.shutdown()


def test_start_background_twice_raises_runtime_error():
    scheduler = Scheduler()
    scheduler.every("1h", lambda: None)
    scheduler.start_background()

    try:
        with pytest.raises(RuntimeError):
            scheduler.start_background()
    finally:
        scheduler.shutdown()


def test_shutdown_waits_for_the_background_thread_to_finish():
    scheduler = Scheduler()
    scheduler.every("1s", lambda: None)
    scheduler.start_background()

    scheduler.shutdown()

    # If shutdown() did not actually join the thread, starting again right
    # after could race with the previous thread still tearing down.
    scheduler.start_background()
    scheduler.shutdown()


def test_shutdown_is_a_safe_no_op_when_nothing_was_started():
    scheduler = Scheduler()
    scheduler.every("1h", lambda: None)

    scheduler.shutdown()  # must not raise even though run()/start_background() never ran


def test_shutdown_is_one_way_jobs_do_not_run_again_after_restart():
    # Documents an intentional limitation: StopSignal only ever transitions
    # false -> true, so a scheduler can't be "resumed" after shutdown(). This
    # matches typical usage (start at app startup, shutdown at app teardown),
    # and avoids the extra complexity/race conditions a restart would need.
    scheduler = Scheduler()
    calls = []
    scheduler.every("1s", lambda: calls.append(time.monotonic()))

    scheduler.start_background()
    time.sleep(1.5)
    scheduler.shutdown()
    calls_before_restart = len(calls)
    assert calls_before_restart >= 1

    scheduler.start_background()
    time.sleep(1.5)
    scheduler.shutdown()

    assert len(calls) == calls_before_restart
