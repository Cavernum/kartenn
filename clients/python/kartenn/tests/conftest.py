import pytest
from hypothesis import HealthCheck, settings

# The machine is shared (k-heavy): the time limits of hypothesis would fail at random.
settings.register_profile(
    "kartenn", deadline=None, suppress_health_check=[HealthCheck.too_slow], max_examples=100
)
settings.load_profile("kartenn")


@pytest.fixture(params=["asyncio", "trio"])
def anyio_backend(request: pytest.FixtureRequest) -> str:
    """Every asynchronous test runs on asyncio and on trio."""
    return str(request.param)
