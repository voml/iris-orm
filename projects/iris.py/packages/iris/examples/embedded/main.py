"""Probe the installed Iris Python binding."""

from __future__ import annotations

from iris import IrisError, capabilities, contract_version, version


def main() -> int:
    """Print the binding version, accepted contract, and capability ids."""
    try:
        print(f"iris binding: {version()}")
        print(f"accepted contract: {contract_version()}")
        print("capabilities: " + ", ".join(capabilities()))
    except IrisError as error:
        print(f"iris binding unavailable: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
