"""
Contains a model for BO4E versions.
"""

import functools
import re
from typing import TYPE_CHECKING

from pydantic import BaseModel

REGEX_VERSION = re.compile(
    r"^v(?P<major>\d{6})\."
    r"(?P<functional>\d+)\."
    r"(?P<technical>\d+)"
    r"(?:-rc(?P<candidate>\d*))?"
    r"(?:\+dev(?P<commit>\w+))?$"
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
    commit: str | None = None
    """
    The commit hash.
    When retrieving the version from a commit which has no tag on it, the version will have the commit hash
    after the last version tag in the history.
    """

    @classmethod
    def from_str(cls, version: str) -> "Version":
        """
        Parse a version string into a Version object e.g. 'v202401.0.1-rc8+dev12asdf34' or 'v202401.0.1'.
        Raises a ValueError if the version string is invalid.
        """
        match = REGEX_VERSION.match(version)
        if match is None:
            raise ValueError(f"Invalid version: {version}")
        return cls.model_validate(match.groupdict())

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

    def is_local_commit(self) -> bool:
        """Check if the version is on a commit without a tag."""
        return self.commit is not None

    def __str__(self) -> str:
        return f"v{self.to_str_without_prefix()}"

    def to_str_without_prefix(self) -> str:
        """Return the version as a string without the 'v' prefix."""
        version = f"{self.major}.{self.functional}.{self.technical}"
        if self.candidate is not None:
            version += f"-rc{self.candidate}"
        if self.commit is not None:
            version += f"+dev{self.commit}"
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
        if self.is_local_commit() and other.is_local_commit():
            raise ValueError("Cannot compare two versions with local commit part.")
        for attr in ["major", "functional", "technical"]:
            if getattr(self, attr) != getattr(other, attr):
                return getattr(self, attr) < getattr(other, attr)  # type: ignore[no-any-return]
        if self.candidate != other.candidate:
            return other.candidate is None or (self.candidate is not None and self.candidate < other.candidate)
        if self.commit != other.commit:
            return self.commit is None  # Implies other.commit is not None
            # I.e. if the other version is at least one commit ahead to this version, it's considered as newer.
        return False  # self == other

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
