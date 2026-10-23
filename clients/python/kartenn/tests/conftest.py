from hypothesis import HealthCheck, settings

# The machine is shared (k-heavy): the time limits of hypothesis would fail at random.
settings.register_profile(
    "kartenn", deadline=None, suppress_health_check=[HealthCheck.too_slow], max_examples=100
)
settings.load_profile("kartenn")
