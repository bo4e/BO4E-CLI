from bo4e_cli.models.meta import Version


class TestVersion:
    def test_from_str(self):
        version = Version.from_str("v202401.0.1-rc8")
        assert version.major == 202401
        assert version.functional == 0
        assert version.technical == 1
        assert version.candidate == 8
        assert version.commit is None

    def test_eq_and_ne(self):
        version1 = Version.from_str("v202401.0.1-rc8")
        version2 = Version.from_str("v202401.0.1-rc8")
        version3 = Version.from_str("v202401.0.1-rc7+dev12984hdac")
        assert version1 == version2
        assert version1 == "v202401.0.1-rc8"
        assert "v202401.0.1-rc8" == version1
        assert version1 != "v202401.0.1-rc7"
        assert "v202401.0.1-rc7+dev12984hdac" != version1
        assert version1 != version3

    def test_str(self):
        version = Version.from_str("v202401.0.1-rc8")
        assert str(version) == "v202401.0.1-rc8"

    def test_is_release_candidate(self):
        version = Version.from_str("v202401.0.1-rc8")
        assert version.is_release_candidate()
        version = Version.from_str("v202401.0.1")
        assert not version.is_release_candidate()

    def test_is_local_commit(self):
        version = Version.from_str("v202401.0.1-rc8")
        assert not version.is_local_commit()
        version = Version.from_str("v202401.0.1+dev12984hdac")
        assert version.is_local_commit()
