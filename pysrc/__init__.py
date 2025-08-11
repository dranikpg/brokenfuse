import os
import json
import datetime
import errno
import json
from dataclasses import dataclass, asdict

FdOrPath = int | str | os.PathLike

def _to_ms(d: datetime.timedelta | int):
    match d:
        case int():
            return d
        case datetime.timedelta():
            return int(d.total_seconds() * 1000)

class Effect:
    COUNTER = 0

    def __init__(self, op: str, data: dict):
        Effect.COUNTER += 1  # Every effect has a unique name
        self.name = f"{type(self).__name__.lower()}-{str(Effect.COUNTER)}"
        self.op = op
        self.data = data

# Delay operations by a fixed amount
class Delay(Effect):
    def __init__(
        self,
        duration: datetime.timedelta | int = 10,
        op: str = "rw",
    ):
        super().__init__(op, {"duration_ms": _to_ms(duration)})


# Issue errors based on one of predefined conditions
class Flakey(Effect):
    Cond = bool | float | tuple

    def __init__(
        self, cond: Cond = True, op: str = "rw", err: int = errno.EIO
    ):  
        data = {}
        match cond:
            case float():
                data = {"prob": cond}
            case (avail, unavail):
                data = {"avail": _to_ms(avail), "unavail": _to_ms(unavail)}
            case bool():
                data = {"always": cond}
        super().__init__(op, data | {"errno": err})


def attach(path: FdOrPath, effect: Effect):
    data = json.dumps({"op": effect.op, **effect.data}).encode('utf-8')
    os.setxattr(path, f"bf.effect.{effect.name}", data)

def remove(path: FdOrPath, effect: Effect):
    os.removexattr(path, f"bf.effect.{effect.name}")

def clear(path: FdOrPath):
    os.removexattr(path, "bf.effect")

def stats(path: FdOrPath):
    return json.loads(os.getxattr(path, "bf.stats").decode('utf8'))
