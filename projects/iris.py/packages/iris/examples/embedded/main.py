"""Probe the installed Iris Python runtime package."""

from __future__ import annotations

from iris import IrisError, contract_version, version


def main() -> int:
    """Print the binding version and the accepted contract."""
    try:
        print(f"iris binding: {version()}")
        print(f"accepted contract: {contract_version()}")
    except IrisError as error:
        print(f"iris binding unavailable: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
