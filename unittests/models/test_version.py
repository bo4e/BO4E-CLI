from bo4e_cli.models.version import Version


class TestVersion:
    def test_from_str(self) -> None:
        version = Version.from_str("v202401.0.1-rc8")
        assert version.major == 202401
        assert version.functional == 0
        assert version.technical == 1
        assert version.candidate == 8
        assert not version.is_dirty()

    def test_eq_and_ne(self) -> None:
        version1 = Version.from_str("v202401.0.1-rc8")
        version2 = Version.from_str("v202401.0.1-rc8")
        version3 = Version.from_str("v202401.0.1-rc7+g12984hdac")
        assert version1 == version2
        assert version1 == "v202401.0.1-rc8"
        assert "v202401.0.1-rc8" == version1
        assert version1 != "v202401.0.1-rc7"
        assert "v202401.0.1-rc7+g12984hdac" != version1
        assert version1 != version3

    def test_str(self) -> None:
        version = Version.from_str("v202401.0.1-rc8")
        assert str(version) == "v202401.0.1-rc8"

    def test_is_release_candidate(self) -> None:
        version = Version.from_str("v202401.0.1-rc8")
        assert version.is_release_candidate()
        version = Version.from_str("v202401.0.1")
        assert not version.is_release_candidate()

    def test_is_local_commit(self) -> None:
        version = Version.from_str("v202401.0.1-rc8")
        assert not version.is_dirty()
        version = Version.from_str("v202401.0.1+g12984hdac")
        assert version.is_dirty()

    def test_total_ordering(self) -> None:
        """
        Test the total ordering of the Version class.
        """
        # pylint: disable=unnecessary-negation
        assert Version.from_str("v202401.1.2") == Version(major=202401, functional=1, technical=2)
        assert Version.from_str("v202401.1.2-rc3") == Version(major=202401, functional=1, technical=2, candidate=3)
        assert Version.from_str("v202401.1.2") < Version.from_str("v202401.1.3")
        assert Version.from_str("v202401.1.2") < Version.from_str("v202401.2.0")
        assert not Version.from_str("v202401.2.0") < Version.from_str("v202401.1.2")
        assert Version.from_str("v202401.2.0") > Version.from_str("v202401.1.2")
        assert Version.from_str("v202401.1.2-rc3") < Version.from_str("v202401.1.2")
        assert Version.from_str("v202401.1.2-rc3") <= Version.from_str("v202401.1.2")
        assert not Version.from_str("v202401.1.2-rc3") >= Version.from_str("v202401.1.2")
        assert Version.from_str("v202401.1.2-rc3") > Version.from_str("v202401.1.1")
        assert Version.from_str("v202401.1.2-rc3") > Version.from_str("v202401.1.2-rc1")
