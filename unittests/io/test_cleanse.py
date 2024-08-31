from pathlib import Path

import pytest

from bo4e_cli.io.cleanse import clear_dir_if_needed


class TestCleanse:
    def test_clear_dir_if_needed(self, tmp_path: Path):
        directory = tmp_path / "test"
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "test.txt").write_text("test")
        sub_dir = directory / "sub_dir"
        sub_dir.mkdir(parents=True, exist_ok=True)
        (sub_dir / "test.txt").write_text("test")

        assert len(list(directory.rglob("*.txt"))) == 2
        clear_dir_if_needed(directory)
        assert not directory.exists()

    def test_clear_dir_if_needed_no_dir(self, tmp_path: Path):
        file = tmp_path / "test.txt"
        file.write_text("test")
        with pytest.raises(ValueError):
            clear_dir_if_needed(file)

    def test_clear_dir_if_needed_empty_dir(self, tmp_path: Path):
        directory = tmp_path / "test"
        directory.mkdir(parents=True, exist_ok=True)
        clear_dir_if_needed(directory)
        assert not directory.exists()

    def test_clear_dir_if_needed_dir_not_exist(self, tmp_path: Path):
        directory = tmp_path / "test"
        assert not directory.exists()
        clear_dir_if_needed(directory)
        assert not directory.exists()
