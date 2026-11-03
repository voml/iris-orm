"""Generation metadata of this client package."""

from __future__ import annotations

GENERATOR_VERSION = "0.1.0"
SCHEMA_FINGERPRINT = "d9ca295a04c4bea047dec4d70904c102f700fce6b1fa6e56d7b0ea68ffa09341"
# VOS schema text this client was generated from (applied by managed adapters).
SCHEMA_SOURCE = "table User {\n    @@user_id: utf8,\n    user_name: utf8,\n    active: bool,\n}\n"
TABLES = ("User", )

__all__ = ["GENERATOR_VERSION", "SCHEMA_FINGERPRINT", "SCHEMA_SOURCE", "TABLES"]
