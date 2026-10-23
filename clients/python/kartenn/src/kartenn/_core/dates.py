"""Dates of the API (REQ-023-9, 22): RFC 3339 with an offset, a date alone in a time zone,
or a relative duration.

The server refuses a date without an offset (REQ-006-5, 400): the library refuses it
first, locally, with the code `client.date-without-offset`.
"""

from __future__ import annotations

import calendar
import re
from datetime import UTC, date, datetime, timedelta, timezone, tzinfo

from kartenn._core.problems import ClientError

__all__ = ["format_rfc3339", "parse_rfc3339", "resolve_date", "to_param"]

# RFC 3339 §5.6 `date-time`, with the `T` or a space (§5.6 NOTE) and `Z` in either case.
_RFC3339 = re.compile(
    r"(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(\.\d+)?([Zz]|[+-]\d{2}:\d{2})?"
)
_DATE = re.compile(r"(\d{4})-(\d{2})-(\d{2})")
# `-7d`, `+1M`, `-90m`: the unit `M` is a month, `m` a minute.
_RELATIVE = re.compile(r"([+-])(\d{1,6})([smhdwMy])")
_UNITS = {"s": "seconds", "m": "minutes", "h": "hours", "d": "days", "w": "weeks"}
_MONTHS_PER_YEAR = 12
_MICRO_DIGITS = 6


def parse_rfc3339(text: str) -> datetime:
    """An RFC 3339 `date-time`; its offset is required."""
    match = _RFC3339.fullmatch(text)
    if match is None:
        msg = f"not an RFC 3339 date-time: {text!r}"
        raise ClientError("client.invalid-date", msg)
    year, month, day, hour, minute, second, fraction, offset = match.groups()
    if offset is None:
        msg = f"an RFC 3339 date-time needs its offset (`Z` or `±hh:mm`): {text!r}"
        raise ClientError("client.date-without-offset", msg)
    micro = int((fraction or ".0")[1:].ljust(_MICRO_DIGITS, "0")[:_MICRO_DIGITS])
    try:
        zone = UTC if offset in "Zz" else _offset(offset)
        return datetime(
            int(year),
            int(month),
            int(day),
            int(hour),
            int(minute),
            int(second),
            micro,
            tzinfo=zone,
        )
    except ValueError as error:
        msg = f"not a valid date-time: {text!r} ({error})"
        raise ClientError("client.invalid-date", msg) from error


def _offset(text: str) -> tzinfo:
    sign = -1 if text[0] == "-" else 1
    hours, minutes = int(text[1:3]), int(text[4:6])
    return timezone(sign * timedelta(hours=hours, minutes=minutes))


def format_rfc3339(moment: datetime) -> str:
    """The RFC 3339 form of an aware datetime, `Z` for UTC, microseconds when present."""
    offset = moment.utcoffset()
    if offset is None:
        msg = f"a naive datetime has no offset: {moment.isoformat()}"
        raise ClientError("client.date-without-offset", msg)
    text = moment.isoformat(timespec="microseconds" if moment.microsecond else "seconds")
    return text[:-6] + "Z" if offset == timedelta(0) else text


def resolve_date(text: str, *, zone: tzinfo, now: datetime) -> datetime:
    """A date of the command line (REQ-023-22): RFC 3339 with an offset, a date alone
    (midnight in `zone`), `now`, or a duration relative to `now` (`-7d`, `+1M`)."""
    if text == "now":
        return now
    if (relative := _RELATIVE.fullmatch(text)) is not None:
        sign, amount, unit = relative.groups()
        count = int(amount) * (-1 if sign == "-" else 1)
        if unit in _UNITS:
            return now + timedelta(**{_UNITS[unit]: count})
        return _add_months(now, count * (_MONTHS_PER_YEAR if unit == "y" else 1))
    if (alone := _DATE.fullmatch(text)) is not None:
        try:
            day = date(*(int(part) for part in alone.groups()))
        except ValueError as error:
            msg = f"not a valid date: {text!r} ({error})"
            raise ClientError("client.invalid-date", msg) from error
        return datetime(day.year, day.month, day.day, tzinfo=zone)
    return parse_rfc3339(text)


def _add_months(moment: datetime, months: int) -> datetime:
    index = moment.year * _MONTHS_PER_YEAR + moment.month - 1 + months
    year, month = divmod(index, _MONTHS_PER_YEAR)
    last = calendar.monthrange(year, month + 1)[1]
    return moment.replace(year=year, month=month + 1, day=min(moment.day, last))


def to_param(value: datetime | str | None) -> str | None:
    """The value of `valid_at` or `as_of`: a datetime is formatted, a string checked."""
    if value is None:
        return None
    if isinstance(value, datetime):
        return format_rfc3339(value)
    parse_rfc3339(value)
    return value
