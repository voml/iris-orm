"""Typed argument objects of the generated client.

Filter keys follow the faithful VOS label of each field: `int`/`float` fields
accept ordering operators, `str` fields accept text predicates, and only
nullable fields accept `is_null` / `is_not_null`.
"""

from __future__ import annotations

from typing import Any, Generic, Mapping, NotRequired, Sequence, TypeVar, TypedDict

_T = TypeVar("_T")

NullFilter = TypedDict("NullFilter", {"is_null": bool, "is_not_null": bool}, total=False)
BooleanFilter = TypedDict("BooleanFilter", {"eq": bool, "not": bool}, total=False)
BigIntFilter = TypedDict(
    "BigIntFilter",
    {"eq": int | str, "not": int | str, "gt": int | str, "gte": int | str, "lt": int | str, "lte": int | str},
    total=False,
)
NumberFilter = TypedDict(
    "NumberFilter",
    {"eq": int | float, "not": int | float, "gt": int | float, "gte": int | float, "lt": int | float, "lte": int | float},
    total=False,
)
StringFilter = TypedDict(
    "StringFilter",
    {"eq": str, "not": str, "contains": str, "starts_with": str, "ends_with": str},
    total=False,
)
UuidFilter = TypedDict("UuidFilter", {"eq": str, "not": str}, total=False)
DecimalFilter = TypedDict(
    "DecimalFilter",
    {"eq": str, "not": str, "gt": str, "gte": str, "lt": str, "lte": str},
    total=False,
)
DateTimeFilter = TypedDict(
    "DateTimeFilter",
    {"eq": str, "not": str, "gt": str, "gte": str, "lt": str, "lte": str},
    total=False,
)
BytesFilter = TypedDict("BytesFilter", {"eq": Mapping[str, Any], "not": Mapping[str, Any]}, total=False)
FileFilter = TypedDict("FileFilter", {"eq": Mapping[str, Any], "not": Mapping[str, Any]}, total=False)


class PatchValue(Generic[_T], TypedDict, total=False):
    """Replace a field with `set`."""

    set: _T


class NullablePatchValue(Generic[_T], TypedDict, total=False):
    """Replace a field with `set` or clear it with `set_null`."""

    set: _T
    set_null: bool



class DocWhereInput(TypedDict, total=False):
    """Condition object of `Doc`; unknown keys are rejected by the generator contract."""

    doc_id: NotRequired[int | BigIntFilter]
    label: NotRequired[str | StringFilter]


class DocSelect(TypedDict, total=False):
    doc_id: NotRequired[bool]
    label: NotRequired[bool]


class DocCreateInput(TypedDict):
    doc_id: int
    label: str
    embedding: Sequence[float]


class DocUpdateInput(TypedDict, total=False):
    label: NotRequired[PatchValue[str]]


class DocRefInput(TypedDict, total=False):
    doc_id: int
