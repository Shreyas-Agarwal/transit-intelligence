from dataclasses import dataclass

@dataclass(frozen=True)
class ZurichConfig:
    stop_prefix: str = "Zürich"
    