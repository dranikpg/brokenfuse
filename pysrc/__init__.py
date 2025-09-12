import os
import json
import datetime
import errno
import json
import typing
import subprocess
from dataclasses import dataclass, asdict

import time

FdOrPath = typing.Annotated[int | str | os.PathLike, "Fd for path to file"]
DurationOrMs = typing.Annotated[datetime.timedelta | int, "Duration or duration in ms"]


def _to_ms(d: DurationOrMs):
    match d:
        case int():
            return d
        case datetime.timedelta():
            return int(d.total_seconds() * 1000)


class Effect:
    """Base class for all attachable effects"""

    _COUNTER = 0

    def __init__(self, op: str, data: dict):
        Effect._COUNTER += 1  # Every effect has a unique name
        self._name = f"{type(self).__name__.lower()}-{str(Effect._COUNTER)}"
        self._op = op
        self._data = data


class Delay(Effect):
    """Delay selected operations by a fixed amount of time"""

    def __init__(
        self,
        duration: datetime.timedelta | int = 10,
        op: str = "rw",
    ):
        super().__init__(op, {"duration_ms": _to_ms(duration)})


class Flakey(Effect):
    """
    Exhibit unreliable behaviour, returing specified error by a selected scenario
    """

    Cond = float | typing.Tuple[DurationOrMs, DurationOrMs]

    def __init__(self, cond: Cond = True, op: str = "rw", err: int = errno.EIO):
        data = {}
        match cond:
            case float():
                data = {"prob": cond}
            case (avail, unavail):
                data = {"avail_ms": _to_ms(avail), "unavail_ms": _to_ms(unavail)}
        super().__init__(op, data | {"errno": err})

    def prob(prob: float, **kwargs):
        """Return error with [0-1] probability"""
        return Flakey(prob, **kwargs)

    def interval(avail: DurationOrMs, unavail: DurationOrMs, **kwargs):
        """Return no errors for `avail` interval, return error for `unavail` interval and cycle this way"""
        return Flakey((avail, unavail), **kwargs)


class MaxSize(Effect):
    """
    Limit maximum size of file or whole subtree, returning ENOSPC on overflow
    """

    def __init__(self, limit: int, op: str = "rw"):
        super().__init__(op, {"limit": limit})


class Heatmap(Effect):
    """
    Heatmap of given operation
    """

    def __init__(self, align: int = 1, op: str = "rw"):
        super().__init__(op, {"align": align})


class Fuse:
    """Manages a running broken fuse"""

    def __init__(self, mount_dir: os.PathLike):
        self._mount_dir = mount_dir

    def start(self):
        cmd = ["brokenfuse", str(self._mount_dir)]
        self._proc = subprocess.Popen(cmd)
        while True:
            try:
                os.getxattr(self._mount_dir, "bf.ino")
                break
            except OSError as e:
                if e.errno != errno.ENOTSUP:
                    raise e

            if self._proc.poll() is not None:
                break

            os.sched_yield()

    def stop(self):
        self._proc.terminate()
        self._proc.wait()

    def __enter__(self):
        self.start()

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()


def attach(path: FdOrPath, effect: Effect):
    data = json.dumps({"op": effect._op, **effect._data}).encode("utf-8")
    os.setxattr(path, f"bf.effect.{effect._name}", data)


def remove(path: FdOrPath, effect: Effect):
    os.removexattr(path, f"bf.effect.{effect.name}")


def clear(path: FdOrPath):
    os.removexattr(path, "bf.effect")


def display(path: FdOrPath, effect: Effect):
    return json.loads(os.getxattr(path, f"bf.effect.{effect.name}").decode("utf8"))


def stats(path: FdOrPath):
    return json.loads(os.getxattr(path, "bf.stats").decode("utf8"))
