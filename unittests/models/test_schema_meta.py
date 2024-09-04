import json
from pathlib import Path

import pytest

from bo4e_cli.models.meta import SchemaMeta
from bo4e_cli.models.schema import SchemaRootObject
from unittests.conftest import TEST_DIR_BO4E_ORIGINAL


class TestSchemaMeta:
    def test_online_src(self) -> None:
        url = "https://raw.githubusercontent.com/BO4E/BO4E-Schemas/v202401.1.0-rc1/src/bo4e_schemas/bo/Angebot.json"
        schema_meta = SchemaMeta(
            name="Angebot",
            module=("bo", "Angebot"),
            src=url,  # type: ignore[arg-type]
        )
        assert str(schema_meta.src_url) == url
        with pytest.raises(ValueError):
            _ = schema_meta.src_path

    def test_local_src(self) -> None:
        path = "src/bo4e_schemas/bo/Angebot.json"
        schema_meta = SchemaMeta(
            name="Angebot",
            module=("bo", "Angebot"),
            src=path,  # type: ignore[arg-type]
        )
        assert schema_meta.src_path == Path(path)
        with pytest.raises(ValueError):
            _ = schema_meta.src_url

    def test_schema_parsed(self) -> None:
        path = TEST_DIR_BO4E_ORIGINAL / "bo/Angebot.json"
        schema_meta = SchemaMeta(
            name="Angebot",
            module=("bo", "Angebot"),
            src=path,
        )
        with open(path, "r", encoding="utf-8") as file:
            content = file.read()
            schema_meta.set_schema_text(content)

        assert schema_meta.get_schema_text() == content
        assert isinstance(schema_meta.get_schema_parsed(), SchemaRootObject)
        assert json.loads(content) == json.loads(schema_meta.get_schema_text())
