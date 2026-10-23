"""Plans: the logic of a call written once for both clients.

A plan is a generator that yields the requests it needs and receives their responses,
then returns its result. The synchronous client runs it with `run`, the asynchronous
one with `arun`: neither the plan nor the core does any input or output.
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable, Generator
from http import HTTPStatus
from typing import TypeVar, cast

from kartenn._core.messages import Request, Response
from kartenn._core.problems import problem_error

__all__ = ["Plan", "arun", "checked", "run"]

T = TypeVar("T")
Plan = Generator[Request, Response, T]


def run(plan: Plan[T], send: Callable[[Request], Response]) -> T:
    try:
        request = next(plan)
        while True:
            request = plan.send(send(request))
    except StopIteration as done:
        return cast("T", done.value)


async def arun(plan: Plan[T], send: Callable[[Request], Awaitable[Response]]) -> T:
    try:
        request = next(plan)
        while True:
            request = plan.send(await send(request))
    except StopIteration as done:
        return cast("T", done.value)


def checked(response: Response) -> Response:
    """The response, or the exception of its problem from 400 on."""
    if response.status >= HTTPStatus.BAD_REQUEST:
        raise problem_error(response)
    return response
