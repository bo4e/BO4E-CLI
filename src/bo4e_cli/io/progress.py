import asyncio
import threading
import time
from itertools import cycle
from typing import TYPE_CHECKING, Callable, Coroutine, Generic, ParamSpec, TypeVar

from rich.progress import Progress, TextColumn

T = TypeVar("T")


async def track_single_async(
    coro: Coroutine[None, None, T],
    *,
    description: str = "Processing",
    finish_description: Callable[[T], str] | None = None,
    appendix_els: tuple[str] = (" .", " ..", " ..."),
    frequency: float = 3,
) -> T:
    """
    Track a single async task by "animating" the description.
    """
    progress = Progress(TextColumn("[progress.description]{task.description}"))
    period_time = 1 / frequency
    with progress:
        task_id = progress.add_task(description)
        async_task = asyncio.create_task(coro)

        async def watch_awaitable():
            description_iter = cycle([f"{description}{appendix}" for appendix in appendix_els])
            while not async_task.done():
                progress.update(task_id, description=next(description_iter))
                await asyncio.sleep(period_time)

        result = (await asyncio.gather(async_task, watch_awaitable()))[0]
        if finish_description is not None:
            progress.update(task_id, description=finish_description(result))
        else:
            progress.update(task_id, description=f"{description} ✅")
        return result


P = ParamSpec("P")


class Routine(Generic[P, T]):
    def __init__(self, function: Callable[P, T], *args: P.args, **kwargs: P.kwargs):
        self._function = function
        self._args = args
        self._kwargs = kwargs

    def __call__(self) -> T:
        return self._function(*self._args, **self._kwargs)


class ThreadWithReturnValue(threading.Thread, Generic[P, T]):
    UNSET = object()

    def __init__(
        self,
        target: Routine[P, T],
        group: None = None,
        name: str | None = None,
    ) -> None:
        super().__init__(group=group, target=target, name=name)
        self._return: T | object = self.UNSET
        if TYPE_CHECKING:
            # This is already done in the super class, but mypy does not recognize it
            self._target: Routine[P, T] = target

    def run(self):
        if self._target is not None:
            self._return = self._target()

    def get_return_value(self) -> T:
        if self.is_alive():
            raise RuntimeError("Thread is still running")
        assert self._return is not self.UNSET
        return self._return


def track_single(
    func: Routine[P, T],
    *,
    description: str = "Processing",
    finish_description: Callable[[T], str] | None = None,
    appendix_els: tuple[str] = (" .", " ..", " ..."),
    frequency: float = 3,
) -> T:
    """
    Track a single _not_ async task by "animating" the description. Uses the threading module to create the
    "animation".
    """
    progress = Progress(TextColumn("[progress.description]{task.description}"))
    period_time = 1 / frequency
    with progress:
        task_id = progress.add_task(description)
        thread = ThreadWithReturnValue[P, T](target=func)

        def watch_thread():
            description_iter = cycle([f"{description}{appendix}" for appendix in appendix_els])
            while thread.is_alive():
                progress.update(task_id, description=next(description_iter))
                time.sleep(period_time)
            thread.join()

        thread_watcher = threading.Thread(target=watch_thread)
        thread.start()
        thread_watcher.start()
        thread.join()
        thread_watcher.join()
        result = thread.get_return_value()
        if finish_description is not None:
            progress.update(task_id, description=finish_description(result))
        else:
            progress.update(task_id, description=f"{description} ✅")
        return result
