"""Generation metadata of this client package."""

from __future__ import annotations

GENERATOR_VERSION = "0.1.0"
SCHEMA_FINGERPRINT = "739bf479308a0ccd581b044d278da106e5dfe0b5fb479456fe093e16d59429da"
# VOS schema text this client was generated from (applied by managed adapters).
SCHEMA_SOURCE = "table Doc {\n    @@doc_id: i64,\n    label: utf8,\n    embedding: vector<3>,\n}\n"
TABLES = ("Doc", )

__all__ = ["GENERATOR_VERSION", "SCHEMA_FINGERPRINT", "SCHEMA_SOURCE", "TABLES"]
