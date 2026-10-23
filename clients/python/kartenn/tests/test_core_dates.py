from datetime import UTC, datetime, timedelta, timezone
from zoneinfo import ZoneInfo

import pytest
from hypothesis import given
from hypothesis import strategies as st

from kartenn._core.dates import format_rfc3339, parse_rfc3339, resolve_date, to_param
from kartenn._core.problems import ClientError

PARIS = ZoneInfo("Europe/Paris")
NOW = datetime(2026, 10, 23, 14, 30, tzinfo=UTC)

offsets = st.integers(min_value=-23 * 60 - 59, max_value=23 * 60 + 59).map(
    lambda minutes: timezone(timedelta(minutes=minutes))
)
moments = st.datetimes(
    min_value=datetime(1, 1, 2),  # noqa: DTZ001 - bounds of hypothesis are naive
    max_value=datetime(9999, 12, 30),  # noqa: DTZ001
    timezones=offsets,
)


@given(moments)
def test_rfc3339_round_trip(moment: datetime) -> None:
    text = format_rfc3339(moment)
    parsed = parse_rfc3339(text)
    assert parsed == moment
    assert parsed.utcoffset() == moment.utcoffset()


def test_utc_is_written_z() -> None:
    assert format_rfc3339(datetime(2026, 1, 1, tzinfo=UTC)) == "2026-01-01T00:00:00Z"
    assert format_rfc3339(datetime(2026, 1, 1, 0, 0, 0, 5, tzinfo=UTC)).endswith(".000005Z")


@pytest.mark.parametrize(
    ("text", "code"),
    [
        ("2026-01-01T00:00:00", "client.date-without-offset"),
        ("2026-13-01T00:00:00Z", "client.invalid-date"),
        ("hier", "client.invalid-date"),
    ],
)
def test_bad_dates_are_refused_locally(text: str, code: str) -> None:
    with pytest.raises(ClientError) as caught:
        parse_rfc3339(text)
    assert caught.value.code == code


def test_naive_datetime_is_refused() -> None:
    with pytest.raises(ClientError, match="naive"):
        format_rfc3339(datetime(2026, 1, 1))  # noqa: DTZ001 - the case refused


def test_offsets_and_fractions() -> None:
    parsed = parse_rfc3339("2026-11-01T00:00:00.5+01:00")
    assert parsed.microsecond == 500_000
    assert parsed.utcoffset() == timedelta(hours=1)
    assert parse_rfc3339("2026-11-01t00:00:00-02:30z".replace("z", "")).utcoffset() == timedelta(
        hours=-2, minutes=-30
    )


def test_date_alone_is_midnight_in_the_zone() -> None:
    resolved = resolve_date("2026-11-01", zone=PARIS, now=NOW)
    assert resolved == datetime(2026, 11, 1, tzinfo=PARIS)
    assert resolved.utcoffset() == timedelta(hours=1)
    with pytest.raises(ClientError):
        resolve_date("2026-02-30", zone=PARIS, now=NOW)


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("now", NOW),
        ("-7d", NOW - timedelta(days=7)),
        ("+90m", NOW + timedelta(minutes=90)),
        ("-2h", NOW - timedelta(hours=2)),
        ("+1w", NOW + timedelta(weeks=1)),
        ("-30s", NOW - timedelta(seconds=30)),
        ("+1M", datetime(2026, 11, 23, 14, 30, tzinfo=UTC)),
        ("-1y", datetime(2025, 10, 23, 14, 30, tzinfo=UTC)),
        ("2026-01-01T00:00:00Z", datetime(2026, 1, 1, tzinfo=UTC)),
    ],
)
def test_relative_dates(text: str, expected: datetime) -> None:
    assert resolve_date(text, zone=PARIS, now=NOW) == expected


def test_month_arithmetic_clamps_the_day() -> None:
    end = datetime(2026, 1, 31, tzinfo=UTC)
    assert resolve_date("+1M", zone=UTC, now=end) == datetime(2026, 2, 28, tzinfo=UTC)
    assert resolve_date("-1M", zone=UTC, now=end) == datetime(2025, 12, 31, tzinfo=UTC)


def test_to_param() -> None:
    assert to_param(None) is None
    assert to_param(datetime(2026, 1, 1, tzinfo=UTC)) == "2026-01-01T00:00:00Z"
    assert to_param("2026-01-01T00:00:00+01:00") == "2026-01-01T00:00:00+01:00"
    with pytest.raises(ClientError):
        to_param("2026-01-01")
