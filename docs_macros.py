import re
import tomllib
from pathlib import Path


_CALENDAR_VERSION = re.compile(
    r"^[1-9][0-9]{3}\.(?:[1-9]|1[0-2])\.(?:0|[1-9][0-9]*)$"
)
_PROJECT_ROOT = Path(__file__).resolve().parent


def _project_version() -> str:
    cargo_manifest = _PROJECT_ROOT / "Cargo.toml"
    with cargo_manifest.open("rb") as manifest:
        version = tomllib.load(manifest)["package"]["version"]

    if not isinstance(version, str) or not _CALENDAR_VERSION.fullmatch(version):
        raise ValueError(
            "Cargo.toml package.version must match YEAR.MONTH.RELEASE"
        )

    return version


def define_env(env) -> None:
    env.variables["retsu_version"] = _project_version()
