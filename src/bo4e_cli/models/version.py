"""
Contains a model for BO4E versions.
"""

import datetime
import functools
import re
from typing import TYPE_CHECKING

from pydantic import BaseModel

REGEX_VERSION = re.compile(
    r"^v(?P<major>\d{6})\."
    r"(?P<functional>\d+)\."
    r"(?P<technical>\d+)"
    r"(?:-rc(?P<candidate>\d*))?"
    r"(?:\+g(?P<commit_part>\w+)"
    r"(?:\.d(?P<dirty_workdir_date_year>\d{4})(?P<dirty_workdir_date_month>\d{2})(?P<dirty_workdir_date_day>\d{2}))?)?$"
)


@functools.total_ordering
class Version(BaseModel):
    """
    A version of the BO4E-Schemas.
    """

    major: int
    functional: int
    technical: int
    candidate: int | None = None
    commit_part: str | None = None
    """
    The commit hash or at least a starting substring of it.
    When retrieving the version from a commit which has no tag on it, the version will have the commit hash
    after the last version tag in the history.
    """
    dirty_workdir_date: datetime.date | None = None
    """
    E.g. if you have local changes in your working directory of the BO4E-python repo, the generation of schemas
    will create versions like ``v202401.0.1.dev0+g12984hdac.d20240101``.
    The last part after the ``.d`` is the date of the last change in the working directory - I believe...
    """

    @classmethod
    def from_str(cls, version: str, *, allow_dirty_version: bool = True) -> "Version":
        """
        Parse a version string into a Version object e.g. 'v202401.0.1-rc8+dev12asdf34' or 'v202401.0.1'.
        Raises a ValueError if the version string is invalid.
        """
        match = REGEX_VERSION.match(version)
        if match is None:
            raise ValueError(f"Invalid version: {version}")
        if not allow_dirty_version and match.group("dev_number") is not None:
            raise ValueError(f"Dirty version not allowed: {version}")
        field_dict = match.groupdict()
        if field_dict["dirty_workdir_date_year"] is not None:
            # If the dirty workdir date is present, parse it into a date object
            field_dict["dirty_workdir_date"] = datetime.date(
                year=int(field_dict.pop("dirty_workdir_date_year")),
                month=int(field_dict.pop("dirty_workdir_date_month")),
                day=int(field_dict.pop("dirty_workdir_date_day")),
            )
        else:
            field_dict["dirty_workdir_date"] = None
            del field_dict["dirty_workdir_date_year"]
            del field_dict["dirty_workdir_date_month"]
            del field_dict["dirty_workdir_date_day"]
        return cls.model_validate(field_dict)

    @classmethod
    def is_valid(cls, version: str) -> bool:
        """
        Check if the version string is valid.
        Returns True if the version string is valid, False otherwise.
        """
        try:
            cls.from_str(version)
            return True
        except ValueError:
            return False

    def is_release_candidate(self) -> bool:
        """Check if the version is a release candidate."""
        return self.candidate is not None

    def is_dirty(self) -> bool:
        """Check if the version is on a commit without a tag or corresponds to a dirty working directory."""
        return self.commit_part is not None

    def __str__(self) -> str:
        return f"v{self.to_str_without_prefix()}"

    def to_str_without_prefix(self) -> str:
        """Return the version as a string without the 'v' prefix."""
        version = f"{self.major}.{self.functional}.{self.technical}"
        if self.candidate is not None:
            version += f"-rc{self.candidate}"
        if self.is_dirty():
            version += f"+g{self.commit_part}"
            if self.dirty_workdir_date is not None:
                version += f".d{self.dirty_workdir_date:%Y%m%d}"
        return version

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Version):
            return super().__eq__(other)
        if isinstance(other, str):
            return str(self) == other
        return NotImplemented

    def __lt__(self, other: "Version") -> bool:
        """
        This method asks: Is this (self) version older than the other version?
        """
        if not isinstance(other, Version):
            return NotImplemented
        if self.is_dirty() and other.is_dirty():
            raise ValueError("Cannot compare two dirty versions. Please rely on analysis with git commands instead.")
        for attr in ["major", "functional", "technical"]:
            if getattr(self, attr) != getattr(other, attr):
                return getattr(self, attr) < getattr(other, attr)  # type: ignore[no-any-return]
        if self.candidate != other.candidate:
            return other.candidate is None or (self.candidate is not None and self.candidate < other.candidate)
        return other.is_dirty()
        # If other version is dirty, the self version cannot be dirty due to the above check.
        # In this case we consider the dirty version as newer.
        # If the other version is not dirty, the self version is either dirty or not. In either case, the self version
        # is not older than the other version.

    if TYPE_CHECKING:  # pragma: no cover

        def __gt__(self, other: "Version") -> bool:
            """
            This method asks: Is this (self) version newer than the other version?
            """

        def __le__(self, other: "Version") -> bool:
            """
            This method asks: Is this (self) version older or equal to the other version?
            """

        def __ge__(self, other: "Version") -> bool:
            """
            This method asks: Is this (self) version newer or equal to the other version?
            """

        def __ne__(self, other: object) -> bool:
            """
            This method asks: Is this (self) version not equal to the other version?
            """

    def bumped_major(self, other: "Version") -> bool:
        """
        Return True if this version is a major bump from the other version.
        """
        return self.major > other.major

    def bumped_functional(self, other: "Version") -> bool:
        """
        Return True if this version is a functional bump from the other version.
        Return False if major bump is detected.
        """
        return not self.bumped_major(other) and self.functional > other.functional

    def bumped_technical(self, other: "Version") -> bool:
        """
        Return True if this version is a technical bump from the other version.
        Return False if major or functional bump is detected.
        """
        return not self.bumped_functional(other) and self.technical > other.technical

    def bumped_candidate(self, other: "Version") -> bool:
        """
        Return True if this version is a candidate bump from the other version.
        Return False if major, functional or technical bump is detected.
        Raises ValueError if one of the versions is not a candidate version.
        """
        if self.candidate is None or other.candidate is None:
            raise ValueError("Cannot compare candidate versions if one of them is not a candidate.")
        return not self.bumped_technical(other) and self.candidate > other.candidate
