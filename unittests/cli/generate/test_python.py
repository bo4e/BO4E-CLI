from importlib import import_module
from pathlib import Path

import pytest
from pydantic import BaseModel
from pydantic.fields import FieldInfo
from typer.testing import CliRunner

from bo4e_cli import app
from unittests.conftest import TEST_DIR_BO4E_REL_REFS, patch_python_path


class TestGeneratePython:
    """
    A class with pytest unit tests.
    """

    @pytest.mark.parametrize(
        "pydantic_version",
        [1, 2],
    )
    def test_generate_python_pydantic(self, pydantic_version: int, tmp_path: Path) -> None:
        output_dir = tmp_path / f"bo4e_pydantic_v{pydantic_version}"
        result = CliRunner().invoke(
            app,
            [
                "generate",
                "-i",
                str(TEST_DIR_BO4E_REL_REFS),
                "-o",
                str(output_dir),
                "-t",
                f"python-pydantic-v{pydantic_version}",
            ],
            catch_exceptions=False,
        )
        assert result.exit_code == 0
        angebot_path = output_dir / "bo/angebot.py"
        assert angebot_path.exists()

        with patch_python_path(output_dir.parent):
            angebot_module = import_module(f"{output_dir.name}.bo.angebot")
            version_module = import_module(f"{output_dir.name}.__version__")
            assert hasattr(angebot_module, "Angebot")
            assert hasattr(version_module, "__version__")
            angebot_class: type[BaseModel] = getattr(angebot_module, "Angebot")
            assert issubclass(angebot_class, BaseModel)
            assert "version" in angebot_class.model_fields
            assert isinstance(angebot_class.model_fields["version"], FieldInfo)
            assert angebot_class.model_fields["version"].default == getattr(version_module, "__version__")

    @pytest.mark.skip(reason="Circular references currently not supported with sqlmodel output")
    def test_generate_python_sql_model(self, tmp_path: Path) -> None:
        output_dir = tmp_path / "bo4e_sqlmodel"
        result = CliRunner().invoke(
            app,
            ["generate", "-i", str(TEST_DIR_BO4E_REL_REFS), "-o", str(output_dir), "-t", "python-sql-model"],
            catch_exceptions=False,
        )
        assert result.exit_code == 0
        angebot_path = output_dir / "bo/angebot.py"
        assert angebot_path.exists()

        with patch_python_path(output_dir.parent):
            angebot_module = import_module(f"{output_dir.name}.bo.angebot")
            version_module = import_module(f"{output_dir.name}.__version__")
            assert hasattr(angebot_module, "Angebot")
            assert hasattr(version_module, "__version__")
            angebot_class: type[BaseModel] = getattr(angebot_module, "Angebot")
            assert issubclass(angebot_class, BaseModel)
            assert "version" in angebot_class.model_fields
            assert isinstance(angebot_class.model_fields["version"], FieldInfo)
            assert angebot_class.model_fields["version"].default == getattr(version_module, "__version__")

            assert "angebotsgeber" in angebot_class.model_fields
            assert "angebotsgeber_id" in angebot_class.model_fields
