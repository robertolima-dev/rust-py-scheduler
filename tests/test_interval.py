import pytest

from rust_py_scheduler import Scheduler


def hello():
    print("hello")


@pytest.mark.parametrize("interval", ["10s", "5m", "1h"])
def test_every_accepts_valid_interval_formats(interval):
    scheduler = Scheduler()
    job_id = scheduler.every(interval, hello)
    assert isinstance(job_id, str)


@pytest.mark.parametrize("interval", ["", "10", "10x", "0s", "-5s", "abcs"])
def test_every_rejects_invalid_interval_formats(interval):
    scheduler = Scheduler()
    with pytest.raises(ValueError):
        scheduler.every(interval, hello)
